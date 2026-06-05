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

    def test_allows_map_depth(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            run_dir = Path(tmp)
            write_run_dir(run_dir, with_command="ast-grep outline map /repo --depth 2")

            self.assertEqual(benchmark.validate_run_dir(run_dir), [])

    def test_rejects_forbidden_outline_format_option(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            run_dir = Path(tmp)
            write_run_dir(
                run_dir,
                with_command="ast-grep outline map /repo --format json",
            )

            issues = benchmark.validate_run_dir(run_dir)

            self.assertIn(
                "sample with-outline run 1: forbidden outline option in raw trace",
                issues,
            )

    def test_allows_find_and_members_outline_subcommands(self) -> None:
        for command in (
            "ast-grep outline find /repo --name RouterGroup",
            "ast-grep outline members /repo --of RouterGroup",
        ):
            with self.subTest(command=command):
                with tempfile.TemporaryDirectory() as tmp:
                    run_dir = Path(tmp)
                    write_run_dir(run_dir, with_command=command)

                    self.assertEqual(benchmark.validate_run_dir(run_dir), [])

    def test_rejects_forbidden_outline_subcommand(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            run_dir = Path(tmp)
            write_run_dir(
                run_dir,
                with_command="ast-grep outline related /repo --symbol RouterGroup",
            )

            issues = benchmark.validate_run_dir(run_dir)

            self.assertIn(
                "sample with-outline run 1: forbidden outline option in raw trace",
                issues,
            )

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
            write_run_dir(run_dir, without_command="ast-grep outline map /repo")

            issues = benchmark.validate_run_dir(run_dir)

            self.assertIn(
                "sample without-outline run 1: outline command used in WITHOUT arm",
                issues,
            )


def write_run_dir(
    run_dir: Path,
    *,
    with_command: str = "ast-grep outline map /repo",
    without_command: str = "rg QuerySet /repo",
    with_tool_result: str = "ok",
    without_tool_result: str = "ok",
    skills: list[str] | None = None,
) -> None:
    (run_dir / "metadata.json").write_text(
        json.dumps({"options": {"repeats": 1}}),
        encoding="utf-8",
    )
    (run_dir / "runs.json").write_text(
        json.dumps(
            [
                run_record("sample", "with-outline", skills=skills),
                run_record("sample", "without-outline"),
            ]
        ),
        encoding="utf-8",
    )
    (run_dir / "alignment.json").write_text(
        json.dumps({"sample": [{"iteration": 1}]}),
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


def write_trace(path: Path, *, command: str, result: str) -> None:
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
