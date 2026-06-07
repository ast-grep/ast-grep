#!/usr/bin/env python3
"""Offline tests for the outline Claude benchmark harness."""

from __future__ import annotations

import importlib.util
import json
import sys
import tempfile
import unittest
from pathlib import Path


SCRIPT = Path(__file__).with_name("outline_claude_benchmark.py")
SPEC = importlib.util.spec_from_file_location("outline_claude_benchmark", SCRIPT)
assert SPEC is not None
benchmark = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = benchmark
assert SPEC.loader is not None
SPEC.loader.exec_module(benchmark)


class ValidateRunDirTest(unittest.TestCase):
    def test_valid_minimal_run(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            run_dir = Path(tmp)
            write_run_dir(run_dir)

            self.assertEqual(benchmark.validate_run_dir(run_dir), [])

    def test_allows_expanded_view(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            run_dir = Path(tmp)
            write_run_dir(run_dir, with_command="ast-grep outline /repo --view expanded")

            self.assertEqual(benchmark.validate_run_dir(run_dir), [])

    def test_rejects_forbidden_outline_json_option(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            run_dir = Path(tmp)
            write_run_dir(
                run_dir,
                with_command="ast-grep outline /repo --json",
            )

            issues = benchmark.validate_run_dir(run_dir)

            self.assertIn(
                "sample with-outline run 1: forbidden outline option in raw trace",
                issues,
            )

    def test_allows_outline_views(self) -> None:
        for command in (
            "ast-grep outline /repo",
            "ast-grep outline /repo --match RouterGroup --view expanded",
            "ast-grep outline /repo --view names",
            "ast-grep outline /repo --view signatures",
            "ast-grep outline /repo --view digest",
            "ast-grep outline /repo --role import",
            "ast-grep outline /repo --role export",
        ):
            with self.subTest(command=command):
                with tempfile.TemporaryDirectory() as tmp:
                    run_dir = Path(tmp)
                    write_run_dir(run_dir, with_command=command)

                    self.assertEqual(benchmark.validate_run_dir(run_dir), [])

    def test_rejects_dirty_claude_init(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            run_dir = Path(tmp)
            write_run_dir(run_dir, skills=["unrelated-skill"])

            issues = benchmark.validate_run_dir(run_dir)

            self.assertIn(
                "sample with-outline run 1: unexpected skills: ['unrelated-skill']",
                issues,
            )

    def test_rejects_outline_in_without_arm(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            run_dir = Path(tmp)
            write_run_dir(run_dir, without_command="ast-grep outline /repo")

            issues = benchmark.validate_run_dir(run_dir)

            self.assertIn(
                "sample without-outline run 1: outline command used in WITHOUT arm",
                issues,
            )

    def test_rejects_empty_runs(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            run_dir = Path(tmp)
            write_run_dir(run_dir, runs=[], alignment={})

            issues = benchmark.validate_run_dir(run_dir)

            self.assertIn("runs.json has no run records", issues)

    def test_rejects_duplicate_run_records(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            run_dir = Path(tmp)
            runs = [
                run_record("sample", "with-outline"),
                run_record("sample", "with-outline"),
                run_record("sample", "without-outline"),
            ]
            write_run_dir(run_dir, runs=runs)

            issues = benchmark.validate_run_dir(run_dir)

            self.assertIn("sample with-outline run 1: duplicate run record", issues)

    def test_alignment_scenario_requires_runs(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            run_dir = Path(tmp)
            write_run_dir(run_dir, runs=[], alignment={"sample": [{"iteration": 1}]})

            issues = benchmark.validate_run_dir(run_dir)

            self.assertIn("sample with-outline run 1: missing run record", issues)
            self.assertIn("sample without-outline run 1: missing run record", issues)

    def test_structured_tool_result_is_checked(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            run_dir = Path(tmp)
            write_run_dir(
                run_dir,
                with_tool_result=[
                    {"type": "text", "text": "Permission to use Bash denied"},
                ],
            )

            issues = benchmark.validate_run_dir(run_dir)

            self.assertIn(
                "sample with-outline run 1: permission denial in raw trace",
                issues,
            )


def write_run_dir(
    run_dir: Path,
    *,
    with_command: str = "ast-grep outline /repo",
    without_command: str = "rg QuerySet /repo",
    with_tool_result: object = "ok",
    without_tool_result: object = "ok",
    skills: list[str] | None = None,
    runs: list[dict[str, object]] | None = None,
    alignment: dict[str, object] | None = None,
) -> None:
    (run_dir / "metadata.json").write_text(
        json.dumps({"options": {"repeats": 1}}),
        encoding="utf-8",
    )
    if runs is None:
        runs = [
            run_record("sample", "with-outline", skills=skills),
            run_record("sample", "without-outline"),
        ]
    if alignment is None:
        alignment = {"sample": [{"iteration": 1}]}
    (run_dir / "runs.json").write_text(
        json.dumps(runs),
        encoding="utf-8",
    )
    (run_dir / "alignment.json").write_text(
        json.dumps(alignment),
        encoding="utf-8",
    )
    write_trace(
        run_dir / "sample-with-outline-1.jsonl",
        command=with_command,
        result=with_tool_result,
    )
    write_trace(
        run_dir / "sample-without-outline-1.jsonl",
        command=without_command,
        result=without_tool_result,
    )


def run_record(
    scenario: str,
    arm: str,
    *,
    skills: list[str] | None = None,
) -> dict[str, object]:
    return {
        "scenario": scenario,
        "arm": arm,
        "iteration": 1,
        "exitCode": 0,
        "error": None,
        "init": {
            "tools": ["Bash", "Glob", "Grep", "Read"],
            "mcpServers": [],
            "slashCommands": [],
            "skills": skills or [],
            "plugins": [],
        },
    }


def write_trace(path: Path, *, command: str, result: object) -> None:
    events = [
        {
            "type": "assistant",
            "message": {
                "content": [
                    {
                        "type": "tool_use",
                        "name": "Bash",
                        "input": {"command": command},
                    }
                ]
            },
        },
        {
            "type": "user",
            "message": {
                "content": [
                    {
                        "type": "tool_result",
                        "content": result,
                    }
                ]
            },
        },
        {
            "type": "result",
            "permission_denials": [],
        },
    ]
    path.write_text(
        "\n".join(json.dumps(event) for event in events) + "\n",
        encoding="utf-8",
    )


if __name__ == "__main__":
    unittest.main()
