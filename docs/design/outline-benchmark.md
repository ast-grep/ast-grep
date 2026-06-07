# Outline Agent Benchmark

This benchmark evaluates `ast-grep outline` as a codebase exploration tool for
AI coding agents. It follows the CodeGraph benchmark shape: ask a real
architecture question about a real repository, run the same headless agent with
and without the structural tool, then compare cost, tokens, wall-clock time,
tool calls, and correctness.

The runner is:

```sh
python3 scripts/outline_claude_benchmark.py
```

It uses real `claude -p` runs. It is not a synthetic grep/read simulation.
The default model is `sonnet`, which currently resolves to Sonnet 4.6 in
Claude Code. Use `--model` to pin a different model for a specific validation.

## Scenario Set

Scenarios live in:

```text
benchmarks/outline-agent-scenarios.json
```

The default runnable set mirrors CodeGraph-style questions for languages that
have bundled outline rules:

| Codebase | Language | Query |
| --- | --- | --- |
| VS Code | TypeScript · ~10k files | How does the extension host communicate with the main process? |
| Excalidraw | TypeScript · ~640 files | How does Excalidraw render and update canvas elements? |
| Django | Python · ~3k files | How does Django's ORM build and execute a query from a QuerySet? |
| Tokio | Rust · ~790 files | How does tokio schedule and run async tasks on its runtime? |
| OkHttp | Java/Kotlin · ~645 files | How does OkHttp process a request through its interceptor chain? |
| Gin | Go · ~110 files | Trace Gin request routing end to end: registration, method-tree lookup, `Context.Next`, and 404/405 fallback chains. |
| Alamofire | Swift · ~110 files | How does Alamofire build, send, and validate a request? |

Each scenario also contains a claim rubric. The rubric is a list of mechanism
claims, and each claim is scored by keyword groups in the agent's final answer
and evidence. This is intentionally stricter than "the answer sounds good" but
cheaper than a second LLM judge.

For these high-level questions, the primary correctness check is
baseline-relative. The `without-outline` answer is treated as the reference
baseline for that run. The benchmark asks two questions about the `with-outline`
answer:

- Coverage: did outline preserve every rubric claim found in the baseline
  answer?
- Precision: did outline avoid adding extra rubric claims that the baseline did
  not support?

This catches both failure modes we care about: missing important baseline
mechanisms and hallucinating additional mechanisms.

For stricter validation, the harness can also run an optional second Claude
judge pass:

```sh
python3 scripts/outline_claude_benchmark.py \
  --scenario gin-middleware-routing \
  --repeats 1 \
  --judge-alignment
```

The judge receives the high-level question, the `without-outline` answer, and
the `with-outline` answer. It treats `without-outline` as the trusted baseline
and returns JSON with coverage, precision, missing baseline mechanisms, extra
unsupported mechanisms, and contradictions. This catches issues outside the
deterministic rubric, such as a plausible but unsupported side mechanism in the
outline answer. It costs one extra `claude -p` call per paired run, so it is
optional rather than the default.

The rubric still matters because it turns both free-form answers into comparable
claim sets. A scenario author can require:

- `terms`: mechanism terms that must appear in the answer.
- `evidenceTerms`: optional file/symbol terms that must also appear, proving the
  answer is grounded in the repository rather than generic knowledge.

For example, the Gin rubric requires claims about `ServeHTTP`,
`handleHTTPRequest`, `RouterGroup.Use`, `combineHandlers`, `Context.Next`, and
the `gin.go`/`routergroup.go`/`context.go` evidence behind them. The harness
reports missing claims so failures are auditable.

## Arms

Each scenario runs in two arms:

| Arm | Setup |
| --- | --- |
| `with-outline` | Claude receives an extra system prompt that presents `ast-grep outline` as a cheaper/faster `Read` when code structure is needed rather than implementation details. It must use outline at least once, should prefer it before reading a whole candidate file, and should avoid using it on large folders. The prompt describes path-sensitive default output and `--match <symbol> --view expanded` for expanding one known symbol. |
| `without-outline` | Claude receives a system prompt forbidding `ast-grep outline` and `sg outline`. Normal Read/Grep/Glob/Bash exploration remains available. |

Both arms can use:

- `Read`
- `Grep`
- `Glob`
- `Bash(rg *)`
- `Bash(find *)`
- `Bash(ls *)`
- `Bash(pwd)`

The only structural-tool difference is `ast-grep outline`.

The runner isolates Claude Code from user/project customization:

- Runs from `/tmp`, not inside the target repository.
- Grants repository access with `--add-dir <repo>`.
- Uses `--setting-sources local`, `--strict-mcp-config`, and
  `--disable-slash-commands`.
- Restricts tools to `Read`, `Grep`, `Glob`, and `Bash`.
- Records the Claude init event in `runs.json`; valid runs should show no MCP
  servers, no slash commands, no skills, and no plugins.

The runner intentionally uses the normal Claude login state so `claude -p` can
run headlessly. It does not rely on the user's project directory or global
plugin/skill configuration; the recorded init event is the enforcement point.

The runner still uses Claude Code `--output-format stream-json` so it can parse
cost, token, time, tool-call, and init-event telemetry. That does not require
the agent's answer to be JSON. The outline prompt is intentionally not a
step-by-step workflow. It tells the agent to use normal search for vocabulary,
behavior terms, and call sites; use outline as a cheaper/faster `Read` when
code structure is needed; avoid outline calls on large folders; and read
concrete code only for evidence.

Runs are paired by scenario and iteration. By default the runner uses
`--arm-order balanced`, which runs `with-outline` first on odd iterations and
`without-outline` first on even iterations. This avoids making all measurements
depend on a single fixed arm order. Other modes are available for debugging:
`with-first`, `without-first`, and `random` with `--seed`.

## Metrics

Metrics are parsed from Claude's JSON stream:

| Metric | Source |
| --- | --- |
| Cost | `result.total_cost_usd` |
| Tokens | `input_tokens + cache_creation_input_tokens + cache_read_input_tokens + output_tokens + reasoning_output_tokens` |
| Time | Local wall-clock duration of the `claude -p` process |
| Tool calls | Count of Claude tool-use events |
| Baseline coverage | Whether `with-outline` covers the claims found in `without-outline` |
| Precision | Whether `with-outline` avoids extra rubric claims not present in `without-outline` |
| LLM judge | Optional whole-answer comparison against the `without-outline` baseline |
| Diagnostic score | Per-arm rubric score, useful for debugging but not the headline correctness metric |

Repository size is recorded separately in `metadata.json` as tracked file count,
source file count, and source line count. Source files are tracked files with
common code extensions: `.ts`, `.tsx`, `.js`, `.jsx`, `.py`, `.rs`, `.java`,
`.kt`, `.kts`, `.go`, and `.swift`.

The final answer is scored as plain text. The harness does not depend on
`--json-schema` or `StructuredOutput`; architecture answers are more reliable
when Claude can answer naturally with evidence. The score is still only as good
as the gold rubric. For publishable results, rubrics should be reviewed by a
human familiar with the codebase or replaced with a maintained gold answer set.

For repeated runs, each metric is summarized independently by median. Baseline
alignment is reported as median coverage, median precision, median F1, and exact
alignment pass count.

## Repository Management

By default, benchmark repositories are cloned under:

```text
target/outline-agent-benchmark/repos/
```

This keeps large third-party repositories out of the ast-grep working tree.
Clone the needed repos with:

```sh
python3 scripts/outline_claude_benchmark.py \
  --sync-repos \
  --repeats 0
```

Clone one repo:

```sh
python3 scripts/outline_claude_benchmark.py \
  --scenario gin-middleware-routing \
  --sync-repos \
  --repeats 0
```

If persistent repo management is desired, the same harness can point at a
submodule directory:

```sh
python3 scripts/outline_claude_benchmark.py \
  --repo-dir benchmarks/repos \
  --sync-repos \
  --repeats 0
```

In that mode, add the benchmark repos as git submodules under
`benchmarks/repos/` and keep the scenario names as the submodule directory
names. The default remains `target/` to avoid committing heavyweight benchmark
fixtures.

## Running

Run one scenario once per arm:

```sh
python3 scripts/outline_claude_benchmark.py \
  --scenario gin-middleware-routing \
  --repeats 1 \
  --max-budget-usd 0.35
```

Run independent agent sessions concurrently:

```sh
python3 scripts/outline_claude_benchmark.py \
  --scenario excalidraw-render-update \
  --repeats 1 \
  --jobs 2
```

`--jobs` defaults to `1` for reproducibility. Higher values reduce elapsed time,
but concurrent Claude sessions can interact with rate limits and cache behavior,
so summaries record the job count in `metadata.json`.

Run the full CodeGraph-style median benchmark:

```sh
python3 scripts/outline_claude_benchmark.py --repeats 4
```

Run with full-answer baseline judging:

```sh
python3 scripts/outline_claude_benchmark.py \
  --repeats 4 \
  --judge-alignment
```

Validate an existing run directory without invoking Claude:

```sh
python3 scripts/outline_claude_benchmark.py \
  --validate-run target/outline-agent-benchmark/<run-id>
```

The validator checks that all runs exited successfully, Claude init was clean,
both arms are present for every iteration, alignments exist, the WITHOUT arm did
not use outline, and the WITH arm did not use forbidden output-shape options
such as `--json` or stale `--format` flags. A run should pass this validator before it is
used for headline effectiveness claims.

The runner also applies the same validation automatically after each benchmark
run writes `metadata.json`, `runs.json`, and `alignment.json`. If validation
fails, it exits nonzero and does not write the headline summary for that run;
the raw artifacts remain available for diagnosis.

The validator has offline regression tests:

```sh
python3 -m unittest scripts/test_outline_claude_benchmark.py
```

Raw traces are written under:

```text
target/outline-agent-benchmark/<run-id>/
```

Each run directory contains:

- `metadata.json`: run options, command line, tool versions, repository
  revisions, and the per-iteration arm order.
- `runs.json`: raw parsed metrics, answers, and per-arm rubric diagnostics.
- `alignment.json`: paired baseline-relative comparison. This lists baseline
  claims, outline claims, covered claims, missed claims, and extra claims per
  iteration.
- `judge-alignment.json`: optional, present only with `--judge-alignment`. This
  lists the second-pass Claude judge verdict for each paired run, including
  missing mechanisms, unsupported extras, and contradictions.

The latest markdown summary is written to:

```text
target/outline-agent-benchmark.md
```

## Completed Run

The last completed full run is:

```text
target/outline-agent-benchmark/4b9c98bc
```

It ran 56 real `claude -p` sessions: 7 repositories, 2 arms, 4 repeats, with
`--jobs 2`. The recorded Claude init events are clean: no MCP servers, no slash
commands, no skills, no plugins, and only `Bash`, `Glob`, `Grep`, and `Read`
were available. The model resolved to `claude-sonnet-4-6`.

Offline validation:

```sh
python3 scripts/outline_claude_benchmark.py --validate-run target/outline-agent-benchmark/4b9c98bc
# valid
```

This is the first clean full run after the leaner prompt and current `outline`
interface. Because it used `--jobs 2`, elapsed time is useful for practical
throughput but should not be compared too strictly with earlier sequential runs.

| Codebase | Language | Code size | Baseline coverage | Cost | Tokens | Time | Tool calls |
| --- | --- | ---: | --- | ---: | ---: | ---: | ---: |
| VS Code | TypeScript | 11,370 src files · 3,279k LOC | cov 100%, prec 100% (3/4) | 35% cheaper | 45% fewer | 12% faster | 11% fewer |
| Excalidraw | TypeScript | 625 src files · 173k LOC | cov 100%, prec 100% (2/4) | 25% cheaper | 26% fewer | 4% slower | 3% more |
| Django | Python | 3,030 src files · 551k LOC | cov 100%, prec 100% (4/4) | 55% cheaper | 67% fewer | 33% faster | 29% fewer |
| Tokio | Rust | 779 src files · 175k LOC | cov 100%, prec 100% (4/4) | 12% cheaper | 38% more | 3% faster | 79% more |
| OkHttp | Java/Kotlin | 640 src files · 135k LOC | cov 100%, prec 100% (4/4) | 40% cheaper | 40% fewer | 5% faster | 45% more |
| Gin | Go | 99 src files · 24k LOC | cov 100%, prec 100% (4/4) | 11% costlier | 39% more | 13% slower | 93% more |
| Alamofire | Swift | 108 src files · 47k LOC | cov 100%, prec 100% (4/4) | even | 48% more | 26% slower | 94% more |

Detailed medians:

| Codebase | Arm | Diagnostic score | Pass rate | Cost USD | Tokens | Seconds | Tool calls |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: |
| VS Code | with-outline | 100% | 4/4 | 0.333012 | 439610 | 115.51 | 31 |
| VS Code | without-outline | 100% | 4/4 | 0.510142 | 797660 | 131.88 | 35 |
| Excalidraw | with-outline | 100% | 4/4 | 0.399023 | 556077 | 142.12 | 39 |
| Excalidraw | without-outline | 100% | 4/4 | 0.533689 | 754892 | 136.90 | 38 |
| Django | with-outline | 100% | 4/4 | 0.211556 | 224086 | 99.86 | 28 |
| Django | without-outline | 100% | 4/4 | 0.470100 | 674898 | 149.89 | 39 |
| Tokio | with-outline | 100% | 4/4 | 0.371982 | 455238 | 128.51 | 26 |
| Tokio | without-outline | 100% | 4/4 | 0.423838 | 329041 | 132.99 | 14 |
| OkHttp | with-outline | 100% | 4/4 | 0.176953 | 146087 | 76.37 | 21 |
| OkHttp | without-outline | 100% | 4/4 | 0.293545 | 243268 | 80.23 | 14 |
| Gin | with-outline | 100% | 4/4 | 0.228370 | 170832 | 90.39 | 14 |
| Gin | without-outline | 100% | 4/4 | 0.204932 | 122971 | 79.93 | 7 |
| Alamofire | with-outline | 100% | 4/4 | 0.334972 | 301384 | 98.73 | 16 |
| Alamofire | without-outline | 100% | 4/4 | 0.336202 | 204146 | 78.38 | 8 |

Baseline alignment details:

| Codebase | Repeated miss or extra |
| --- | --- |
| VS Code | `with-outline` missed the ExtHost/MainThread counterpart-service claim in 1/4 paired runs. |
| Excalidraw | Mixed: `with-outline` missed the pointer/event mutation claim once, and added it once when the baseline omitted it. |

## Token And Cost Analysis

The token numbers are not just "tool calls went down." Claude Code charges and
reports separate token classes: fresh input, cache creation, cache reads, and
output. In this run, outline usually lowered cost on larger repositories by
reducing broad file reads and cache-creation volume, but it did not universally
reduce total tokens or tool calls.

Examples:

- VS Code, Django, and OkHttp are the clearest wins: outline preserved or nearly
  preserved baseline alignment while cutting median cost by 35%, 55%, and 40%.
- Excalidraw shows the remaining correctness sensitivity. The median result was
  cheaper and lower-token, but two paired runs were not exactly aligned because
  the pointer/event mutation mechanism moved in or out of one arm's answer.
- Tokio shows that lower cost can coexist with higher counted tokens. The
  outline arm had more cache-read tokens and tool calls, but lower median cost.
- Gin and Alamofire are the weak cases. They were already small enough for
  grep/read exploration, so outline added tool calls and did not improve
  correctness.

The main remaining failure mode is agent ergonomics, not extraction correctness:
outline helps when it replaces broad reads, but hurts when it is added on top of
already-sufficient grep/read exploration. The prompt now frames outline as a
cheaper/faster `Read` for code structure, but smaller repositories still need
better agent judgment about when to skip or minimize structural calls.
