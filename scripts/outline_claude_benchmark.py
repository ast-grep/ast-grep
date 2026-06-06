#!/usr/bin/env python3
"""End-to-end Claude benchmark for ast-grep outline on real repositories.

This benchmark mirrors the CodeGraph-style setup:

* same high-level architecture question per repo
* WITH arm: Claude gets an outline-specific system prompt and `ast-grep outline`
* WITHOUT arm: Claude is forbidden from using outline
* built-in Read/Grep/Glob/Bash exploration remains available to both
* metrics come from the real `claude -p` run
* correctness is checked by comparing WITH answers against WITHOUT baselines
"""

from __future__ import annotations

import argparse
import datetime as dt
import json
import os
import random
import re
import statistics
import subprocess
import sys
import time
import uuid
from dataclasses import dataclass
from pathlib import Path
from typing import Any


ROOT = Path(__file__).resolve().parents[1]
DEFAULT_SCENARIOS = ROOT / "benchmarks" / "outline-agent-scenarios.json"
DEFAULT_REPO_DIR = ROOT / "target" / "outline-agent-benchmark" / "repos"
NEUTRAL_CWD = Path("/tmp")
FORBIDDEN_OUTLINE_RE = re.compile(
    r"\b(?:ast-grep|sg)\s+outline\b[^\n]*--format\b"
)


OUTLINE_SYSTEM_PROMPT = """\
You are answering an architecture question about a codebase.

Use normal file discovery first: `Glob`, `Grep`, `rg`, `find`, and `ls` are for
finding candidate files and names. After you have candidate files or focused
directories, you must use `ast-grep outline` at least once to get a compact
structural view before reading deeply or answering. Do not start with
`ast-grep outline map` on the repository root.

Useful commands:
- `ast-grep outline map <path>`: skim top-level definitions in a file or
  focused subtree.
- `ast-grep outline members <path> --of <symbol>`: list methods, fields, and
  nested types for a known class, struct, trait, interface, or module symbol.
- `ast-grep outline imports <path>`: see dependencies for a file or focused subtree.
- `ast-grep outline exports <path>`: see public API exported by a file or subtree.

Use these outline command forms as shown. Do not request JSON or other formats.

Typical workflow:
1. Use `Glob`/`Grep`/`rg` to find likely files and vocabulary.
2. Use `map` on focused files or small focused directories, not the repo root.
   Default `map` is top-level only; use `members` for nested details in a known
   class, struct, trait, interface, or module.
3. Use `members` after identifying a concrete parent symbol.
4. Use `imports` and `exports` when dependency direction or public API matters.
5. Read each important file once, preferably around the relevant symbols, and
   use grep for missing line evidence instead of rereading the same file.

Cite concrete file, symbol, and line evidence in the final answer.
"""


WITHOUT_OUTLINE_SYSTEM_PROMPT = """\
You are answering an architecture question about a codebase.

Do not run `ast-grep outline` or `sg outline`. Use normal code exploration
tools such as grep, glob, shell, and file reads, and cite specific evidence from
files/symbols in the final answer.
"""


@dataclass(frozen=True)
class RequiredClaim:
    claim: str
    terms: tuple[tuple[str, ...], ...]
    evidence_terms: tuple[tuple[str, ...], ...]


@dataclass(frozen=True)
class Scenario:
    name: str
    codebase: str
    language: str
    approx_files: str
    repo: str
    query: str
    outline_default_rules: bool
    correctness_threshold: float
    required_claims: tuple[RequiredClaim, ...]


@dataclass(frozen=True)
class RubricResult:
    score: float
    passed_claims: list[str]
    missing_claims: list[str]
    correct: bool


@dataclass(frozen=True)
class AgentRun:
    scenario: str
    codebase: str
    arm: str
    iteration: int
    repo_path: str
    exit_code: int
    duration_seconds: float
    cost_usd: float | None
    tokens: int | None
    tool_calls: int
    answer: str
    key_points: list[str]
    evidence: list[str]
    rubric: RubricResult
    init: dict[str, Any]
    error: str | None


@dataclass(frozen=True)
class ArmSummary:
    scenario: str
    arm: str
    cost_usd: float | None
    tokens: float | None
    duration_seconds: float
    tool_calls: float
    score: float
    pass_count: int
    run_count: int


@dataclass(frozen=True)
class CoverageResult:
    iteration: int
    coverage: float
    precision: float
    f1: float
    covered_claims: list[str]
    missed_claims: list[str]
    extra_claims: list[str]
    baseline_claims: list[str]
    outline_claims: list[str]
    correct: bool


@dataclass(frozen=True)
class CoverageSummary:
    coverage: float
    precision: float
    f1: float
    pass_count: int
    run_count: int


@dataclass(frozen=True)
class JudgeResult:
    scenario: str
    iteration: int
    duration_seconds: float
    cost_usd: float | None
    tokens: int | None
    coverage: float
    precision: float
    verdict: str
    missing: list[str]
    extra: list[str]
    contradictions: list[str]
    error: str | None


def load_scenarios(path: Path) -> list[Scenario]:
    data = json.loads(path.read_text(encoding="utf-8"))
    scenarios: list[Scenario] = []
    for item in data:
        scenarios.append(
            Scenario(
                name=item["name"],
                codebase=item["codebase"],
                language=item["language"],
                approx_files=item["approxFiles"],
                repo=item["repo"],
                query=item["query"],
                outline_default_rules=bool(item.get("outlineDefaultRules", True)),
                correctness_threshold=float(item.get("correctnessThreshold", 0.8)),
                required_claims=tuple(
                    RequiredClaim(
                        claim=claim["claim"],
                        terms=tuple(tuple(group) for group in claim["terms"]),
                        evidence_terms=tuple(tuple(group) for group in claim.get("evidenceTerms", [])),
                    )
                    for claim in item["requiredClaims"]
                ),
            )
        )
    return scenarios


def repo_path(repo_dir: Path, scenario: Scenario) -> Path:
    return repo_dir / scenario.name


def sync_repo(repo_dir: Path, scenario: Scenario, refresh: bool) -> Path:
    path = repo_path(repo_dir, scenario)
    if path.exists():
        if refresh:
            run_command(["git", "fetch", "--depth", "1", "origin"], path).check_returncode()
            run_command(["git", "reset", "--hard", "origin/HEAD"], path).check_returncode()
        return path
    repo_dir.mkdir(parents=True, exist_ok=True)
    run_command(["git", "clone", "--depth", "1", scenario.repo, str(path)], ROOT).check_returncode()
    return path


def arms_for_iteration(order: str, iteration: int, rng: random.Random) -> list[str]:
    arms = ["with-outline", "without-outline"]
    if order == "with-first":
        return arms
    if order == "without-first":
        return list(reversed(arms))
    if order == "balanced":
        return arms if iteration % 2 == 1 else list(reversed(arms))
    if order == "random":
        shuffled = list(arms)
        rng.shuffle(shuffled)
        return shuffled
    raise ValueError(f"unknown arm order: {order}")


def build_command(
    scenario: Scenario,
    arm: str,
    repo: Path,
    model: str | None,
    max_budget_usd: float | None,
) -> list[str]:
    prompt = (
        f"Repository root: {repo}\n\n"
        f"{scenario.query}\n\n"
        "Answer as a senior engineer explaining the mechanism. Keep the answer "
        "concise but complete. Use paths under the repository root when calling "
        "tools. Include concrete file, symbol, or line evidence."
    )
    command = [
        "claude",
        "-p",
        "--output-format",
        "stream-json",
        "--verbose",
        "--strict-mcp-config",
        "--setting-sources",
        "local",
        "--disable-slash-commands",
        "--tools",
        "Read,Grep,Glob,Bash",
        "--permission-mode",
        "auto",
        "--add-dir",
        str(repo),
        "--no-session-persistence",
        "--system-prompt",
        OUTLINE_SYSTEM_PROMPT if arm == "with-outline" else WITHOUT_OUTLINE_SYSTEM_PROMPT,
        prompt,
    ]
    command[1:1] = allowed_tool_args(arm)
    if model:
        command[1:1] = ["--model", model]
    if max_budget_usd is not None:
        command[1:1] = ["--max-budget-usd", str(max_budget_usd)]
    if arm == "with-outline":
        command[1:1] = [
            "--disallowedTools",
            "Bash(ast-grep outline *--format*)",
        ]
    if arm == "without-outline":
        command[1:1] = [
            "--disallowedTools",
            "Bash(ast-grep outline *)",
            "Bash(sg outline *)",
        ]
    return command


def allowed_tool_args(arm: str) -> list[str]:
    tools = [
        "Read",
        "Grep",
        "Glob",
        "Bash(rg *)",
        "Bash(find *)",
        "Bash(ls *)",
        "Bash(pwd)",
    ]
    if arm == "with-outline":
        tools.append("Bash(ast-grep outline *)")
    return ["--allowedTools", ",".join(tools)]


def run_command(
    command: list[str],
    cwd: Path,
    timeout_seconds: int | None = None,
) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        command,
        cwd=cwd,
        env=os.environ.copy(),
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        timeout=timeout_seconds,
        check=False,
    )


def run_judge_command(
    command: list[str],
    cwd: Path,
    prompt: str,
    timeout_seconds: int | None = None,
) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        command,
        cwd=cwd,
        env=os.environ.copy(),
        input=prompt,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        timeout=timeout_seconds,
        check=False,
    )


def command_text(command: list[str], cwd: Path) -> str | None:
    completed = run_command(command, cwd)
    if completed.returncode != 0:
        text = completed.stderr.strip() or completed.stdout.strip()
        return f"error: {text}" if text else f"error: exit {completed.returncode}"
    return completed.stdout.strip() or completed.stderr.strip()


def repo_revision(repo: Path) -> dict[str, str | None]:
    return {
        "path": str(repo),
        "commit": command_text(["git", "rev-parse", "HEAD"], repo),
        "remote": command_text(["git", "remote", "get-url", "origin"], repo),
        "branch": command_text(["git", "rev-parse", "--abbrev-ref", "HEAD"], repo),
    }


def write_metadata(
    output: Path,
    scenarios: list[Scenario],
    repo_dir: Path,
    run_id: str,
    args: argparse.Namespace,
    arm_plan: list[dict[str, Any]],
) -> None:
    metadata = {
        "runId": run_id,
        "startedAtUtc": dt.datetime.now(dt.UTC).isoformat(),
        "argv": sys.argv,
        "options": {
            "repeats": args.repeats,
            "armOrder": args.arm_order,
            "seed": args.seed,
            "includeUnsupportedOutline": args.include_unsupported_outline,
            "model": args.model,
            "maxBudgetUsd": args.max_budget_usd,
            "judgeAlignment": args.judge_alignment,
            "judgeModel": args.judge_model,
            "judgeMaxBudgetUsd": args.judge_max_budget_usd,
            "repoDir": str(repo_dir),
            "neutralCwd": str(NEUTRAL_CWD),
            "claudeSettingSources": "local",
            "disableSlashCommands": True,
            "toolSet": "Read,Grep,Glob,Bash",
        },
        "versions": {
            "claude": command_text(["claude", "--version"], ROOT),
            "astGrep": command_text(["ast-grep", "--version"], ROOT),
            "python": command_text([sys.executable, "--version"], ROOT),
            "git": command_text(["git", "--version"], ROOT),
        },
        "repositories": {
            scenario.name: repo_revision(repo_path(repo_dir, scenario))
            for scenario in scenarios
            if repo_path(repo_dir, scenario).exists()
        },
        "armPlan": arm_plan,
    }
    output.write_text(json.dumps(metadata, indent=2), encoding="utf-8")


def run_agent(
    scenario: Scenario,
    arm: str,
    iteration: int,
    repo: Path,
    raw_dir: Path,
    model: str | None,
    max_budget_usd: float | None,
) -> AgentRun:
    command = build_command(scenario, arm, repo, model, max_budget_usd)
    started = time.perf_counter()
    completed = run_command(command, NEUTRAL_CWD)
    duration = time.perf_counter() - started
    raw_path = raw_dir / f"{scenario.name}-{arm}-{iteration}.jsonl"
    raw_path.write_text(completed.stdout, encoding="utf-8")
    parsed = parse_claude_output(completed.stdout)
    rubric = score_answer(scenario, parsed["answer"], parsed["key_points"], parsed["evidence"])
    error = parsed["error"]
    if completed.returncode != 0 and error is None:
        error = completed.stderr.strip() or f"claude exited {completed.returncode}"
    return AgentRun(
        scenario=scenario.name,
        codebase=scenario.codebase,
        arm=arm,
        iteration=iteration,
        repo_path=str(repo),
        exit_code=completed.returncode,
        duration_seconds=duration,
        cost_usd=parsed["cost_usd"],
        tokens=parsed["tokens"],
        tool_calls=parsed["tool_calls"],
        answer=parsed["answer"],
        key_points=parsed["key_points"],
        evidence=parsed["evidence"],
        rubric=rubric,
        init=parsed["init"],
        error=error,
    )


def parse_claude_output(stdout: str) -> dict[str, Any]:
    cost_usd: float | None = None
    tokens: int | None = None
    tool_calls = 0
    error: str | None = None
    answer = ""
    key_points: list[str] = []
    evidence: list[str] = []
    final_text = ""
    init: dict[str, Any] = {}

    for line in stdout.splitlines():
        if not line.strip():
            continue
        try:
            event = json.loads(line)
        except json.JSONDecodeError:
            continue
        tool_calls += count_tool_calls(event)
        event_type = event.get("type")
        if event_type == "system" and event.get("subtype") == "init":
            init = normalize_init_event(event)
        elif event_type == "result":
            cost_usd = event.get("total_cost_usd", cost_usd)
            structured_output = event.get("structured_output")
            if isinstance(structured_output, dict):
                answer, key_points, evidence = normalize_answer(structured_output)
            usage = event.get("usage")
            if isinstance(usage, dict):
                tokens = sum_token_usage(usage)
            result = event.get("result")
            if isinstance(result, str):
                final_text = result
            if event.get("is_error") and isinstance(result, str):
                error = result
        elif event_type == "assistant":
            structured = extract_structured_output_tool_input(event)
            if structured is not None:
                answer, key_points, evidence = structured
            text = extract_text(event)
            if text:
                final_text = text

    if not answer and final_text:
        answer = final_text

    return {
        "cost_usd": cost_usd,
        "tokens": tokens,
        "tool_calls": tool_calls,
        "answer": answer,
        "key_points": key_points,
        "evidence": evidence,
        "init": init,
        "error": error,
    }


def normalize_init_event(event: dict[str, Any]) -> dict[str, Any]:
    return {
        "cwd": event.get("cwd"),
        "tools": event.get("tools"),
        "mcpServers": event.get("mcp_servers"),
        "model": event.get("model"),
        "permissionMode": event.get("permissionMode"),
        "slashCommands": event.get("slash_commands"),
        "skills": event.get("skills"),
        "plugins": event.get("plugins"),
        "apiKeySource": event.get("apiKeySource"),
        "claudeCodeVersion": event.get("claude_code_version"),
    }


def validate_init_event(run: AgentRun) -> list[str]:
    issues: list[str] = []
    init = run.init
    if not init:
        return ["missing Claude init event"]
    if init.get("mcpServers") not in ([], {}, None):
        issues.append(f"unexpected MCP servers: {init.get('mcpServers')}")
    if init.get("slashCommands") not in ([], None):
        issues.append(f"unexpected slash commands: {init.get('slashCommands')}")
    if init.get("skills") not in ([], None):
        issues.append(f"unexpected skills: {init.get('skills')}")
    if init.get("plugins") not in ([], None):
        issues.append(f"unexpected plugins: {init.get('plugins')}")
    tools = set(init.get("tools") or [])
    unexpected = tools - {"Read", "Grep", "Glob", "Bash"}
    if unexpected:
        issues.append(f"unexpected tools: {sorted(unexpected)}")
    return issues


def parse_claude_json_result(stdout: str) -> tuple[str, float | None, int | None, str | None]:
    try:
        event = json.loads(stdout)
    except json.JSONDecodeError:
        return "", None, None, f"judge output was not Claude JSON: {stdout[:200]}"
    result = event.get("result", "")
    if not isinstance(result, str):
        result = ""
    usage = event.get("usage")
    tokens = sum_token_usage(usage) if isinstance(usage, dict) else None
    cost = event.get("total_cost_usd")
    error = None
    if event.get("is_error"):
        error = result or "judge returned an error"
    return result, cost, tokens, error


def parse_json_object(text: str) -> dict[str, Any] | None:
    stripped = text.strip()
    if stripped.startswith("```"):
        stripped = stripped.strip("`").removeprefix("json").strip()
    try:
        value = json.loads(stripped)
        return value if isinstance(value, dict) else None
    except json.JSONDecodeError:
        pass
    start = stripped.find("{")
    end = stripped.rfind("}")
    if start == -1 or end == -1 or end <= start:
        return None
    try:
        value = json.loads(stripped[start : end + 1])
    except json.JSONDecodeError:
        return None
    return value if isinstance(value, dict) else None


def count_tool_calls(event: dict[str, Any]) -> int:
    count = 0
    message = event.get("message")
    if isinstance(message, dict):
        content = message.get("content")
        if isinstance(content, list):
            count += sum(1 for item in content if item.get("type") == "tool_use")
    item = event.get("item")
    if isinstance(item, dict) and item.get("type") in {"tool_use", "command_execution"}:
        count += 1
    return count


def sum_token_usage(usage: dict[str, Any]) -> int:
    keys = [
        "input_tokens",
        "cache_creation_input_tokens",
        "cache_read_input_tokens",
        "output_tokens",
        "reasoning_output_tokens",
    ]
    return sum(int(usage.get(key, 0) or 0) for key in keys)


def extract_text(event: dict[str, Any]) -> str:
    message = event.get("message")
    if not isinstance(message, dict):
        return ""
    content = message.get("content")
    if not isinstance(content, list):
        return ""
    return "".join(
        item.get("text", "")
        for item in content
        if isinstance(item, dict) and item.get("type") == "text"
    )


def extract_structured_output_tool_input(event: dict[str, Any]) -> tuple[str, list[str], list[str]] | None:
    message = event.get("message")
    if not isinstance(message, dict):
        return None
    content = message.get("content")
    if not isinstance(content, list):
        return None
    for item in content:
        if not isinstance(item, dict):
            continue
        if item.get("type") != "tool_use" or item.get("name") != "StructuredOutput":
            continue
        input_value = item.get("input")
        if isinstance(input_value, dict):
            return normalize_answer(input_value)
    return None


def normalize_answer(value: dict[str, Any]) -> tuple[str, list[str], list[str]]:
    answer = str(value.get("answer", ""))
    key_points = value.get("keyPoints", [])
    evidence = value.get("evidence", [])
    if not isinstance(key_points, list):
        key_points = []
    if not isinstance(evidence, list):
        evidence = []
    return answer, [str(item) for item in key_points], [str(item) for item in evidence]


def score_answer(
    scenario: Scenario,
    answer: str,
    key_points: list[str],
    evidence: list[str],
) -> RubricResult:
    text = normalize_text("\n".join([answer, *key_points, *evidence]))
    passed: list[str] = []
    missing: list[str] = []
    for claim in scenario.required_claims:
        has_terms = all(any(normalize_text(term) in text for term in group) for group in claim.terms)
        has_evidence = all(
            any(normalize_text(term) in text for term in group)
            for group in claim.evidence_terms
        )
        if has_terms and has_evidence:
            passed.append(claim.claim)
        else:
            missing.append(claim.claim)
    score = len(passed) / len(scenario.required_claims) if scenario.required_claims else 0.0
    return RubricResult(
        score=score,
        passed_claims=passed,
        missing_claims=missing,
        correct=score >= scenario.correctness_threshold,
    )


def normalize_text(text: str) -> str:
    return " ".join(text.lower().replace("_", " ").replace("-", " ").split())


def pct_cost(with_value: float | None, without_value: float | None) -> str:
    if with_value is None or without_value in (None, 0):
        return "n/a"
    delta = (without_value - with_value) / without_value * 100.0
    if abs(delta) < 0.5:
        return "even"
    return f"{abs(delta):.0f}% {'cheaper' if delta > 0 else 'costlier'}"


def pct_savings(with_value: float | None, without_value: float | None) -> str:
    if with_value is None or without_value in (None, 0):
        return "n/a"
    delta = (without_value - with_value) / without_value * 100.0
    if abs(delta) < 0.5:
        return "even"
    return f"{abs(delta):.0f}% {'fewer' if delta > 0 else 'more'}"


def pct_time(with_value: float, without_value: float) -> str:
    if without_value == 0:
        return "n/a"
    delta = (without_value - with_value) / without_value * 100.0
    if abs(delta) < 0.5:
        return "even"
    return f"{abs(delta):.0f}% {'faster' if delta > 0 else 'slower'}"


def write_summary(
    scenarios: list[Scenario],
    runs: list[AgentRun],
    output: Path,
    repeats: int,
    judge_results: list[JudgeResult] | None = None,
) -> None:
    scenario_by_name = {scenario.name: scenario for scenario in scenarios}
    by_key: dict[tuple[str, str], list[AgentRun]] = {}
    for run in runs:
        by_key.setdefault((run.scenario, run.arm), []).append(run)
    scenario_names: list[str] = []
    for run in runs:
        if run.scenario not in scenario_names:
            scenario_names.append(run.scenario)

    lines = [
        "# Claude Outline Agent Benchmark Results",
        "",
        f"Median of {repeats} `claude -p` runs per arm.",
        "",
        "| Codebase | Language | Baseline coverage | Cost | Tokens | Time | Tool calls |",
        "| --- | --- | --- | ---: | ---: | ---: | ---: |",
    ]
    rows: list[tuple[Scenario, ArmSummary, ArmSummary, CoverageSummary]] = []
    for name in scenario_names:
        scenario = scenario_by_name[name]
        with_runs = by_key[(name, "with-outline")]
        without_runs = by_key[(name, "without-outline")]
        with_summary = summarize_arm(with_runs)
        without_summary = summarize_arm(without_runs)
        coverage_summary = summarize_coverage(compare_to_baseline(with_runs, without_runs))
        rows.append((scenario, with_summary, without_summary, coverage_summary))
        lines.append(
            f"| {scenario.codebase} | {scenario.language} · {scenario.approx_files} | "
            f"{format_coverage(coverage_summary)} | "
            f"{pct_cost(with_summary.cost_usd, without_summary.cost_usd)} | "
            f"{pct_savings(with_summary.tokens, without_summary.tokens)} | "
            f"{pct_time(with_summary.duration_seconds, without_summary.duration_seconds)} | "
            f"{pct_savings(with_summary.tool_calls, without_summary.tool_calls)} |"
        )

    lines.extend(
        [
            "",
            "Detailed medians:",
            "",
            "| Codebase | Arm | Diagnostic score | Pass rate | Cost USD | Tokens | Seconds | Tool calls |",
            "| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: |",
        ]
    )
    for scenario, with_summary, without_summary, _coverage_summary in rows:
        for summary in (with_summary, without_summary):
            lines.append(
                f"| {scenario.codebase} | {summary.arm} | {summary.score:.0%} | "
                f"{summary.pass_count}/{summary.run_count} | "
                f"{format_optional(summary.cost_usd, 6)} | {format_optional(summary.tokens, 0)} | "
                f"{summary.duration_seconds:.2f} | {summary.tool_calls:.0f} |"
            )
    lines.extend(
        [
            "",
            "Baseline alignment details:",
            "",
            "| Codebase | Coverage | Precision | F1 | Exact aligned |",
            "| --- | ---: | ---: | ---: | ---: |",
        ]
    )
    for scenario, _with_summary, _without_summary, coverage_summary in rows:
        lines.append(
            f"| {scenario.codebase} | {coverage_summary.coverage:.0%} | "
            f"{coverage_summary.precision:.0%} | {coverage_summary.f1:.0%} | "
            f"{coverage_summary.pass_count}/{coverage_summary.run_count} |"
        )
    if judge_results:
        scenario_by_name = {scenario.name: scenario for scenario in scenarios}
        lines.extend(
            [
                "",
                "LLM baseline judge details:",
                "",
                "| Codebase | Coverage | Precision | Verdict | Missing | Extra | Contradictions |",
                "| --- | ---: | ---: | --- | ---: | ---: | ---: |",
            ]
        )
        for result in judge_results:
            scenario = scenario_by_name[result.scenario]
            verdict = result.verdict or ("error" if result.error else "")
            lines.append(
                f"| {scenario.codebase} | {result.coverage:.0%} | "
                f"{result.precision:.0%} | {verdict} | "
                f"{len(result.missing)} | {len(result.extra)} | "
                f"{len(result.contradictions)} |"
            )
    output.write_text("\n".join(lines) + "\n", encoding="utf-8")


def summarize_arm(runs: list[AgentRun]) -> ArmSummary:
    if not runs:
        raise ValueError("cannot summarize an empty run list")
    return ArmSummary(
        scenario=runs[0].scenario,
        arm=runs[0].arm,
        cost_usd=median_optional(run.cost_usd for run in runs),
        tokens=median_optional(run.tokens for run in runs),
        duration_seconds=float(statistics.median(run.duration_seconds for run in runs)),
        tool_calls=float(statistics.median(run.tool_calls for run in runs)),
        score=float(statistics.median(run.rubric.score for run in runs)),
        pass_count=sum(1 for run in runs if run.rubric.correct),
        run_count=len(runs),
    )


def compare_to_baseline(
    with_runs: list[AgentRun],
    without_runs: list[AgentRun],
) -> list[CoverageResult]:
    without_by_iteration = {run.iteration: run for run in without_runs}
    results: list[CoverageResult] = []
    for with_run in sorted(with_runs, key=lambda run: run.iteration):
        baseline = without_by_iteration.get(with_run.iteration)
        if baseline is None:
            continue
        baseline_claims = list(baseline.rubric.passed_claims)
        outline_claims = list(with_run.rubric.passed_claims)
        baseline_set = set(baseline_claims)
        with_set = set(with_run.rubric.passed_claims)
        covered = [claim for claim in baseline_claims if claim in with_set]
        missed = [claim for claim in baseline_claims if claim not in with_set]
        extra = [claim for claim in outline_claims if claim not in baseline_set]
        coverage = len(covered) / len(baseline_claims) if baseline_claims else 1.0
        precision = len(covered) / len(outline_claims) if outline_claims else (1.0 if not baseline_claims else 0.0)
        f1 = (
            2 * coverage * precision / (coverage + precision)
            if coverage + precision > 0
            else 0.0
        )
        results.append(
            CoverageResult(
                iteration=with_run.iteration,
                coverage=coverage,
                precision=precision,
                f1=f1,
                covered_claims=covered,
                missed_claims=missed,
                extra_claims=extra,
                baseline_claims=baseline_claims,
                outline_claims=outline_claims,
                correct=not missed and not extra,
            )
        )
    return results


def summarize_coverage(results: list[CoverageResult]) -> CoverageSummary:
    if not results:
        return CoverageSummary(coverage=0.0, precision=0.0, f1=0.0, pass_count=0, run_count=0)
    return CoverageSummary(
        coverage=float(statistics.median(result.coverage for result in results)),
        precision=float(statistics.median(result.precision for result in results)),
        f1=float(statistics.median(result.f1 for result in results)),
        pass_count=sum(1 for result in results if result.correct),
        run_count=len(results),
    )


def format_coverage(summary: CoverageSummary) -> str:
    if summary.run_count == 0:
        return "n/a"
    return (
        f"cov {summary.coverage:.0%}, prec {summary.precision:.0%} "
        f"({summary.pass_count}/{summary.run_count})"
    )


def median_optional(values: Any) -> float | None:
    present = [float(value) for value in values if value is not None]
    if not present:
        return None
    return float(statistics.median(present))


def format_correctness(with_summary: ArmSummary, without_summary: ArmSummary) -> str:
    if (
        with_summary.pass_count == with_summary.run_count
        and without_summary.pass_count == without_summary.run_count
    ):
        return "both"
    return (
        f"outline={with_summary.pass_count}/{with_summary.run_count} "
        f"({with_summary.score:.0%}), "
        f"baseline={without_summary.pass_count}/{without_summary.run_count} "
        f"({without_summary.score:.0%})"
    )


def format_optional(value: float | int | None, digits: int) -> str:
    if value is None:
        return "n/a"
    if isinstance(value, int) or digits == 0:
        return str(int(value))
    return f"{value:.{digits}f}"


def run_to_json(run: AgentRun) -> dict[str, Any]:
    return {
        "scenario": run.scenario,
        "codebase": run.codebase,
        "arm": run.arm,
        "iteration": run.iteration,
        "repoPath": run.repo_path,
        "exitCode": run.exit_code,
        "durationSeconds": run.duration_seconds,
        "costUsd": run.cost_usd,
        "tokens": run.tokens,
        "toolCalls": run.tool_calls,
        "answer": run.answer,
        "keyPoints": run.key_points,
        "evidence": run.evidence,
        "init": run.init,
        "rubric": {
            "score": run.rubric.score,
            "correct": run.rubric.correct,
            "passedClaims": run.rubric.passed_claims,
            "missingClaims": run.rubric.missing_claims,
        },
        "error": run.error,
    }


def judge_result_to_json(result: JudgeResult) -> dict[str, Any]:
    return {
        "scenario": result.scenario,
        "iteration": result.iteration,
        "durationSeconds": result.duration_seconds,
        "costUsd": result.cost_usd,
        "tokens": result.tokens,
        "coverage": result.coverage,
        "precision": result.precision,
        "verdict": result.verdict,
        "missing": result.missing,
        "extra": result.extra,
        "contradictions": result.contradictions,
        "error": result.error,
    }


def alignment_to_json(result: CoverageResult) -> dict[str, Any]:
    return {
        "iteration": result.iteration,
        "coverage": result.coverage,
        "precision": result.precision,
        "f1": result.f1,
        "correct": result.correct,
        "baselineClaims": result.baseline_claims,
        "outlineClaims": result.outline_claims,
        "coveredClaims": result.covered_claims,
        "missedClaims": result.missed_claims,
        "extraClaims": result.extra_claims,
    }


def build_judge_prompt(scenario: Scenario, baseline: AgentRun, outline: AgentRun) -> str:
    return f"""\
You are judging an architecture benchmark.

Question:
{scenario.query}

Treat the WITHOUT-outline answer as the trusted baseline. Compare the
WITH-outline answer against it. Do not penalize wording, order, or extra detail
that is consistent with the baseline. Do penalize:
- missing important baseline mechanisms
- extra mechanisms not supported by the baseline
- contradictions with the baseline

Return only JSON with this shape:
{{
  "coverage": 0.0,
  "precision": 0.0,
  "verdict": "aligned | missing | extra | contradicted | mixed",
  "missing": ["important baseline claims absent from WITH-outline"],
  "extra": ["WITH-outline claims not supported by baseline"],
  "contradictions": ["WITH-outline claims that contradict baseline"]
}}

WITHOUT-outline baseline answer:
{baseline.answer}

WITH-outline answer:
{outline.answer}
"""


def run_alignment_judge(
    scenarios: list[Scenario],
    runs: list[AgentRun],
    model: str | None,
    max_budget_usd: float | None,
    raw_dir: Path,
) -> list[JudgeResult]:
    scenario_by_name = {scenario.name: scenario for scenario in scenarios}
    by_key: dict[tuple[str, str], list[AgentRun]] = {}
    for run in runs:
        by_key.setdefault((run.scenario, run.arm), []).append(run)

    results: list[JudgeResult] = []
    for (scenario_name, arm), with_runs in sorted(by_key.items()):
        if arm != "with-outline":
            continue
        scenario = scenario_by_name[scenario_name]
        without_by_iteration = {
            run.iteration: run
            for run in by_key.get((scenario_name, "without-outline"), [])
        }
        for with_run in sorted(with_runs, key=lambda run: run.iteration):
            baseline = without_by_iteration.get(with_run.iteration)
            if baseline is None:
                continue
            prompt = build_judge_prompt(scenario, baseline, with_run)
            command = [
                "claude",
                "-p",
                "--output-format",
                "json",
                "--strict-mcp-config",
                "--setting-sources",
                "local",
                "--disable-slash-commands",
                "--tools",
                "",
                "--permission-mode",
                "auto",
                "--add-dir",
                with_run.repo_path,
                "--no-session-persistence",
            ]
            if model:
                command[1:1] = ["--model", model]
            if max_budget_usd is not None:
                command[1:1] = ["--max-budget-usd", str(max_budget_usd)]
            started = time.perf_counter()
            completed = run_judge_command(command, NEUTRAL_CWD, prompt)
            duration = time.perf_counter() - started
            raw_path = raw_dir / f"{scenario.name}-judge-{with_run.iteration}.json"
            raw_path.write_text(completed.stdout, encoding="utf-8")
            result_text, cost, tokens, error = parse_claude_json_result(completed.stdout)
            value = parse_json_object(result_text)
            if completed.returncode != 0 and error is None:
                error = completed.stderr.strip() or f"judge exited {completed.returncode}"
            if value is None:
                value = {}
                if error is None:
                    error = f"judge result was not JSON: {result_text[:200]}"
            results.append(
                JudgeResult(
                    scenario=scenario.name,
                    iteration=with_run.iteration,
                    duration_seconds=duration,
                    cost_usd=cost,
                    tokens=tokens,
                    coverage=float(value.get("coverage", 0.0) or 0.0),
                    precision=float(value.get("precision", 0.0) or 0.0),
                    verdict=str(value.get("verdict", "error" if error else "")),
                    missing=[str(item) for item in value.get("missing", []) if item is not None],
                    extra=[str(item) for item in value.get("extra", []) if item is not None],
                    contradictions=[
                        str(item)
                        for item in value.get("contradictions", [])
                        if item is not None
                    ],
                    error=error,
                )
            )
    return results


def write_alignment_json(runs: list[AgentRun], output: Path) -> None:
    by_key: dict[tuple[str, str], list[AgentRun]] = {}
    for run in runs:
        by_key.setdefault((run.scenario, run.arm), []).append(run)
    scenario_names: list[str] = []
    for run in runs:
        if run.scenario not in scenario_names:
            scenario_names.append(run.scenario)
    data: dict[str, list[dict[str, Any]]] = {}
    for name in scenario_names:
        data[name] = [
            alignment_to_json(result)
            for result in compare_to_baseline(
                by_key.get((name, "with-outline"), []),
                by_key.get((name, "without-outline"), []),
            )
        ]
    output.write_text(json.dumps(data, indent=2), encoding="utf-8")


def write_judge_json(results: list[JudgeResult], output: Path) -> None:
    output.write_text(
        json.dumps([judge_result_to_json(result) for result in results], indent=2),
        encoding="utf-8",
    )


def validate_run_dir(run_dir: Path) -> list[str]:
    issues: list[str] = []
    metadata_path = run_dir / "metadata.json"
    runs_path = run_dir / "runs.json"
    alignment_path = run_dir / "alignment.json"
    for path in (metadata_path, runs_path, alignment_path):
        if not path.exists():
            issues.append(f"missing {path.name}")
    if issues:
        return issues

    try:
        metadata = json.loads(metadata_path.read_text(encoding="utf-8"))
        runs = json.loads(runs_path.read_text(encoding="utf-8"))
        alignment = json.loads(alignment_path.read_text(encoding="utf-8"))
    except json.JSONDecodeError as err:
        return [f"invalid JSON: {err}"]

    if not isinstance(runs, list):
        issues.append("runs.json is not a list")
        return issues
    if not isinstance(alignment, dict):
        issues.append("alignment.json is not an object")
        return issues

    repeats = int(metadata.get("options", {}).get("repeats", 0) or 0)
    by_key: dict[tuple[str, str, int], dict[str, Any]] = {}
    scenarios: set[str] = set()
    for raw_run in runs:
        if not isinstance(raw_run, dict):
            issues.append("runs.json contains a non-object run")
            continue
        scenario = str(raw_run.get("scenario", ""))
        arm = str(raw_run.get("arm", ""))
        iteration = int(raw_run.get("iteration", 0) or 0)
        scenarios.add(scenario)
        by_key[(scenario, arm, iteration)] = raw_run
        issues.extend(validate_run_record(raw_run))
        issues.extend(validate_raw_trace(run_dir, scenario, arm, iteration))

    for scenario in sorted(scenarios):
        for iteration in range(1, repeats + 1):
            for arm in ("with-outline", "without-outline"):
                if (scenario, arm, iteration) not in by_key:
                    issues.append(f"{scenario} {arm} run {iteration}: missing run record")
        aligned = alignment.get(scenario)
        if not isinstance(aligned, list):
            issues.append(f"{scenario}: missing alignment entries")
        elif repeats and len(aligned) != repeats:
            issues.append(
                f"{scenario}: expected {repeats} alignment entries, found {len(aligned)}"
            )

    return issues


def validate_run_record(raw_run: dict[str, Any]) -> list[str]:
    prefix = (
        f"{raw_run.get('scenario')} {raw_run.get('arm')} "
        f"run {raw_run.get('iteration')}"
    )
    issues: list[str] = []
    if raw_run.get("exitCode") != 0:
        issues.append(f"{prefix}: nonzero exit {raw_run.get('exitCode')}")
    if raw_run.get("error"):
        issues.append(f"{prefix}: recorded error {raw_run.get('error')}")
    init = raw_run.get("init")
    if not isinstance(init, dict):
        issues.append(f"{prefix}: missing init event")
        return issues
    issues.extend(f"{prefix}: {issue}" for issue in validate_init_dict(init))
    return issues


def validate_init_dict(init: dict[str, Any]) -> list[str]:
    issues: list[str] = []
    if init.get("mcpServers") not in ([], {}, None):
        issues.append(f"unexpected MCP servers: {init.get('mcpServers')}")
    if init.get("slashCommands") not in ([], None):
        issues.append(f"unexpected slash commands: {init.get('slashCommands')}")
    if init.get("skills") not in ([], None):
        issues.append(f"unexpected skills: {init.get('skills')}")
    if init.get("plugins") not in ([], None):
        issues.append(f"unexpected plugins: {init.get('plugins')}")
    tools = set(init.get("tools") or [])
    unexpected = tools - {"Read", "Grep", "Glob", "Bash"}
    if unexpected:
        issues.append(f"unexpected tools: {sorted(unexpected)}")
    return issues


def validate_raw_trace(run_dir: Path, scenario: str, arm: str, iteration: int) -> list[str]:
    prefix = f"{scenario} {arm} run {iteration}"
    path = run_dir / f"{scenario}-{arm}-{iteration}.jsonl"
    if not path.exists():
        return [f"{prefix}: missing raw trace {path.name}"]
    commands, tool_results, permission_denials = parse_trace_tool_events(path)
    issues: list[str] = []
    if permission_denials or any("Permission to use Bash" in result for result in tool_results):
        issues.append(f"{prefix}: permission denial in raw trace")
    if any("unexpected argument" in result for result in tool_results):
        issues.append(f"{prefix}: CLI argument error in raw trace")
    outline_commands = [
        command
        for command in commands
        if "ast-grep outline" in command or "sg outline" in command
    ]
    if arm == "with-outline":
        if not any("ast-grep outline" in command for command in outline_commands):
            issues.append(f"{prefix}: no ast-grep outline use in WITH arm")
        if any(FORBIDDEN_OUTLINE_RE.search(command) for command in outline_commands):
            issues.append(f"{prefix}: forbidden outline option in raw trace")
    else:
        if outline_commands:
            issues.append(f"{prefix}: outline command used in WITHOUT arm")
    return issues


def parse_trace_tool_events(path: Path) -> tuple[list[str], list[str], list[dict[str, Any]]]:
    commands: list[str] = []
    tool_results: list[str] = []
    permission_denials: list[dict[str, Any]] = []
    for line in path.read_text(encoding="utf-8").splitlines():
        if not line.strip():
            continue
        try:
            event = json.loads(line)
        except json.JSONDecodeError:
            continue
        message = event.get("message")
        if isinstance(message, dict):
            content = message.get("content")
            if isinstance(content, list):
                for item in content:
                    if not isinstance(item, dict):
                        continue
                    if item.get("type") == "tool_use":
                        input_value = item.get("input")
                        if isinstance(input_value, dict):
                            command = input_value.get("command")
                            if isinstance(command, str):
                                commands.append(command)
                    elif item.get("type") == "tool_result":
                        result = item.get("content")
                        if isinstance(result, str):
                            tool_results.append(result)
        if event.get("type") == "result":
            denials = event.get("permission_denials")
            if isinstance(denials, list):
                permission_denials.extend(
                    denial for denial in denials if isinstance(denial, dict)
                )
    return commands, tool_results, permission_denials


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--scenarios", type=Path, default=DEFAULT_SCENARIOS)
    parser.add_argument("--scenario", action="append")
    parser.add_argument(
        "--validate-run",
        type=Path,
        default=None,
        help="Validate an existing raw benchmark run directory and exit without running Claude.",
    )
    parser.add_argument(
        "--include-unsupported-outline",
        action="store_true",
        help="Also run scenarios whose languages do not have bundled outline rules. Diagnostic only.",
    )
    parser.add_argument("--repo-dir", type=Path, default=DEFAULT_REPO_DIR)
    parser.add_argument("--sync-repos", action="store_true")
    parser.add_argument("--refresh-repos", action="store_true")
    parser.add_argument("--repeats", type=int, default=4)
    parser.add_argument(
        "--arm-order",
        choices=["balanced", "with-first", "without-first", "random"],
        default="balanced",
        help="Order arms within each paired iteration. Default alternates by iteration.",
    )
    parser.add_argument("--seed", type=int, default=0)
    parser.add_argument("--model", default="sonnet")
    parser.add_argument("--max-budget-usd", type=float, default=None)
    parser.add_argument(
        "--judge-alignment",
        action="store_true",
        help="Run a second no-tool Claude pass to compare WITH answers against WITHOUT baseline answers.",
    )
    parser.add_argument("--judge-model", default=None)
    parser.add_argument("--judge-max-budget-usd", type=float, default=None)
    parser.add_argument(
        "--raw-dir",
        type=Path,
        default=ROOT / "target" / "outline-agent-benchmark",
    )
    parser.add_argument(
        "--summary",
        type=Path,
        default=ROOT / "target" / "outline-agent-benchmark.md",
    )
    args = parser.parse_args()

    if args.validate_run is not None:
        issues = validate_run_dir(args.validate_run)
        if issues:
            print(f"{args.validate_run}: invalid")
            for issue in issues:
                print(f"- {issue}")
            return 1
        print(f"{args.validate_run}: valid")
        return 0

    scenarios = load_scenarios(args.scenarios)
    if args.scenario:
        wanted = set(args.scenario)
        scenarios = [scenario for scenario in scenarios if scenario.name in wanted]
        missing = wanted - {scenario.name for scenario in scenarios}
        if missing:
            print(f"unknown scenario(s): {', '.join(sorted(missing))}", file=sys.stderr)
            return 2
    unsupported = [
        scenario
        for scenario in scenarios
        if not scenario.outline_default_rules
    ]
    if unsupported and not args.include_unsupported_outline:
        for scenario in unsupported:
            print(
                f"skipping {scenario.name}: {scenario.language} has no bundled outline rules "
                f"(add bundled outline rules before using it for effectiveness claims)",
                file=sys.stderr,
            )
        scenarios = [scenario for scenario in scenarios if scenario.outline_default_rules]
    if not scenarios:
        print("no runnable scenarios selected", file=sys.stderr)
        return 2

    if args.sync_repos:
        for scenario in scenarios:
            path = sync_repo(args.repo_dir, scenario, args.refresh_repos)
            print(f"{scenario.name}: {path}")
        if args.repeats == 0:
            return 0

    run_id = uuid.uuid4().hex[:8]
    raw_dir = args.raw_dir / run_id
    raw_dir.mkdir(parents=True, exist_ok=True)
    runs: list[AgentRun] = []
    arm_plan: list[dict[str, Any]] = []
    rng = random.Random(args.seed)
    for scenario in scenarios:
        repo = repo_path(args.repo_dir, scenario)
        if not repo.exists():
            print(
                f"missing repo for {scenario.name}: {repo}\n"
                f"run with --sync-repos first",
                file=sys.stderr,
            )
            return 2
        for iteration in range(1, args.repeats + 1):
            arms = arms_for_iteration(args.arm_order, iteration, rng)
            arm_plan.append(
                {
                    "scenario": scenario.name,
                    "iteration": iteration,
                    "arms": arms,
                }
            )
            for arm in arms:
                run = run_agent(
                    scenario,
                    arm,
                    iteration,
                    repo,
                    raw_dir,
                    args.model,
                    args.max_budget_usd,
                )
                runs.append(run)
                print(
                    f"{scenario.name} {arm} run {iteration}: "
                    f"score={run.rubric.score:.0%}, {run.duration_seconds:.2f}s, "
                    f"tokens={run.tokens}, cost={run.cost_usd}, tools={run.tool_calls}",
                    flush=True,
                )
                init_issues = validate_init_event(run)
                if init_issues:
                    print(
                        f"{scenario.name} {arm} run {iteration}: invalid Claude init: "
                        + "; ".join(init_issues),
                        file=sys.stderr,
                    )
                    return 3
                if run.exit_code != 0:
                    print(
                        f"{scenario.name} {arm} run {iteration}: Claude exited "
                        f"{run.exit_code}: {run.error}",
                        file=sys.stderr,
                    )
                    return run.exit_code or 1

    metadata_path = raw_dir / "metadata.json"
    write_metadata(metadata_path, scenarios, args.repo_dir, run_id, args, arm_plan)
    result_path = raw_dir / "runs.json"
    result_path.write_text(json.dumps([run_to_json(run) for run in runs], indent=2), encoding="utf-8")
    alignment_path = raw_dir / "alignment.json"
    write_alignment_json(runs, alignment_path)
    if runs:
        validation_issues = validate_run_dir(raw_dir)
        if validation_issues:
            print(f"{raw_dir}: invalid", file=sys.stderr)
            for issue in validation_issues:
                print(f"- {issue}", file=sys.stderr)
            print(f"metadata: {metadata_path}")
            print(f"raw results: {result_path}")
            print(f"alignment: {alignment_path}")
            return 4
    judge_path: Path | None = None
    judge_results: list[JudgeResult] | None = None
    if args.judge_alignment:
        judge_results = run_alignment_judge(
            scenarios,
            runs,
            args.judge_model or args.model,
            args.judge_max_budget_usd,
            raw_dir,
        )
        judge_path = raw_dir / "judge-alignment.json"
        write_judge_json(judge_results, judge_path)
    write_summary(scenarios, runs, args.summary, args.repeats, judge_results)
    print(f"metadata: {metadata_path}")
    print(f"raw results: {result_path}")
    print(f"alignment: {alignment_path}")
    if judge_path:
        print(f"judge alignment: {judge_path}")
    print(f"summary: {args.summary}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
