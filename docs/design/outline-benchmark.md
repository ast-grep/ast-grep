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
| `with-outline` | Claude receives an extra system prompt telling it to use normal file discovery first, then `ast-grep outline` for compact structure on candidate files/subtrees. The arm uses one v1 structural primitive with focused views: default outline output, `--of`, `--show imports`, and `--show exports`. The default view is top-level, and `--of` is preferred after a parent symbol is known. |
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

The runner still uses Claude Code `--output-format stream-json` so it can parse
cost, token, time, tool-call, and init-event telemetry. That does not require
the agent's answer to be JSON. The outline prompt tells the agent to discover
files with normal tools first, avoid whole-repo outline calls, treat the default view as
a top-level file signature, use `--of` only after identifying a parent symbol, and then
read implementation details with concrete line evidence.

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
such as JSON `--format` flags. A run should pass this validator before it is
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
target/outline-agent-benchmark/263beca4
```

It ran 56 real `claude -p` sessions: 7 repositories, 2 arms, 4 repeats. The
recorded Claude init events are clean: no MCP servers, no slash commands, no
skills, no plugins, and only `Bash`, `Glob`, `Grep`, and `Read` were available.
The model resolved to `claude-sonnet-4-6`.

Important caveat: this run is complete and useful as a diagnostic result, but it
is not a clean final effectiveness claim. Several `with-outline` traces used
older unsupported outline options; some attempts failed with CLI errors and
some were hidden by shell redirection. The runner has since been tightened to
deny unsupported output-shape options. A later Django smoke run
(`target/outline-agent-benchmark/52511cf5`) verified the
tighter guardrails: no denied commands, clean init, and 100% alignment. The user
stopped further full benchmark execution, so the table below remains the last
complete full run rather than the clean-final run.

Offline validation confirms the distinction:

```sh
python3 scripts/outline_claude_benchmark.py --validate-run target/outline-agent-benchmark/263beca4
# invalid: old unsupported outline options and CLI errors

python3 scripts/outline_claude_benchmark.py --validate-run target/outline-agent-benchmark/52511cf5
# valid
```

| Codebase | Language | Baseline coverage | Cost | Tokens | Time | Tool calls |
| --- | --- | --- | ---: | ---: | ---: | ---: |
| VS Code | TypeScript · ~10k files | cov 75%, prec 100% (1/4) | 23% cheaper | 29% fewer | 17% slower | 9% fewer |
| Excalidraw | TypeScript · ~640 files | cov 100%, prec 88% (1/4) | 15% cheaper | 21% fewer | 15% faster | 6% fewer |
| Django | Python · ~3k files | cov 100%, prec 100% (4/4) | 28% cheaper | 38% fewer | 19% faster | 19% fewer |
| Tokio | Rust · ~790 files | cov 100%, prec 100% (4/4) | 28% cheaper | 31% fewer | 10% faster | 2% fewer |
| OkHttp | Java/Kotlin · ~645 files | cov 100%, prec 100% (4/4) | 58% cheaper | 58% fewer | 29% faster | 11% fewer |
| Gin | Go · ~110 files | cov 80%, prec 100% (0/4) | 26% cheaper | 5% fewer | 16% slower | 83% more |
| Alamofire | Swift · ~110 files | cov 88%, prec 100% (1/4) | 31% cheaper | 3% more | 26% slower | 133% more |

Detailed medians:

| Codebase | Arm | Diagnostic score | Pass rate | Cost USD | Tokens | Seconds | Tool calls |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: |
| VS Code | with-outline | 75% | 4/4 | 0.269998 | 328176 | 118.72 | 26 |
| VS Code | without-outline | 100% | 4/4 | 0.352706 | 459632 | 101.39 | 28 |
| Excalidraw | with-outline | 100% | 4/4 | 0.397854 | 499028 | 105.07 | 30 |
| Excalidraw | without-outline | 88% | 4/4 | 0.470027 | 629372 | 123.36 | 32 |
| Django | with-outline | 100% | 4/4 | 0.187020 | 187271 | 80.24 | 21 |
| Django | without-outline | 100% | 4/4 | 0.260207 | 303049 | 99.42 | 26 |
| Tokio | with-outline | 100% | 4/4 | 0.245409 | 262238 | 104.68 | 24 |
| Tokio | without-outline | 100% | 4/4 | 0.342576 | 382046 | 116.64 | 24 |
| OkHttp | with-outline | 100% | 4/4 | 0.120506 | 103991 | 55.07 | 12 |
| OkHttp | without-outline | 100% | 4/4 | 0.284164 | 247326 | 77.21 | 14 |
| Gin | with-outline | 80% | 4/4 | 0.105430 | 88892 | 52.00 | 11 |
| Gin | without-outline | 100% | 4/4 | 0.141686 | 93872 | 44.73 | 6 |
| Alamofire | with-outline | 88% | 4/4 | 0.232696 | 260906 | 89.55 | 21 |
| Alamofire | without-outline | 100% | 4/4 | 0.336230 | 253466 | 71.23 | 9 |

Baseline-relative misses:

| Codebase | Repeated miss or extra |
| --- | --- |
| VS Code | `with-outline` missed the ExtHost/MainThread counterpart-service claim in 3/4 paired runs. |
| Excalidraw | Mixed: outline sometimes added the pointer/update claim when baseline did not, and missed it once. |
| Gin | `with-outline` missed the method-tree route matching claim in all 4 paired runs. |
| Alamofire | Mixed: outline missed or added the interceptor/adapt/retry claim depending on the paired baseline. |

## Token And Cost Analysis

The token numbers are not just "tool calls went down." Claude Code charges and
reports separate token classes: fresh input, cache creation, cache reads, and
output. In this run, outline usually lowered expensive cache creation and large
read/grep discovery volume, which made cost lower even when wall-clock or tool
count did not improve.

Examples:

- OkHttp was the cleanest win in the completed diagnostic run. Under the older
  prompt, outline jumped to `RealCall` and `RealInterceptorChain`, cutting
  median tokens by 58%, cost by 58%, and time by 29% while preserving 100%
  alignment.
- Django and Tokio also show good structural targeting. Outline cut median
  tokens by 38% and 31% respectively with 100% alignment.
- VS Code was cheaper and lower-token, but correctness suffered. The outline
  arm often found the transport/protocol mechanism but underreported the
  ExtHost/MainThread service-counterpart layer.
- Gin shows lower cost but worse ergonomics. Outline used more tool calls than
  the baseline and still failed to mention method-tree route matching. The
  baseline found `trees`, `getValue`, and `tree.go` more directly.
- Alamofire shows why token count and cost can diverge. The outline arm had 3%
  more median tokens and many more tool calls, but 31% lower median cost because
  its token mix had lower cache-creation cost than the baseline. It was still
  slower and less consistently aligned.

The main failure mode is not that `outline` is inherently too expensive. The
failure mode is agent over-exploration: broad outline calls and follow-up reads
that are not always guided to the missing mechanism. The tightened prompt and
command guardrails address the command-shape failure, but a clean full rerun is
still needed before publishing headline claims.
