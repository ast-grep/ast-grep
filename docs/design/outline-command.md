# Outline Command Design

## Summary

Add a new top-level `outline` subcommand to ast-grep:

```sh
sg outline <query> [OPTIONS] [PATHS]...
```

The command is an AI-agent-friendly structural code intelligence primitive. It parses
source files through ast-grep/tree-sitter and answers navigation questions such as:

- What symbols are in these files?
- What does this file import?
- What does this module export?
- What members belong to this class, struct, enum, interface, or module?

For v1, the supported query surface is deliberately small: `map`, `imports`,
`exports`, and `members`. Other useful workflows, such as reviewing changed files after
an edit, should be composed from these primitives and existing tools like `git diff`.

The symbol kind model should follow the Language Server Protocol `SymbolKind` enum so
the output can be consumed by editors and downstream tools without ast-grep-specific
symbol categories.

## Design Principle

AI agents do not primarily need a pretty document outline. They need bounded,
machine-readable answers that help decide which file or range to open next.

The ideal command should therefore be query-oriented, not only display-oriented:

```sh
sg outline map crates/cli/src --format jsonl --budget 200
sg outline imports crates --to ast-grep-config --format jsonl
sg outline exports crates/config/src --format jsonl
sg outline members crates/cli/src/lib.rs --of Commands --format json
```

Each query should answer a navigation question directly. Avoid making agents compose
many low-level filters for common code exploration tasks.

## Goals

- Give AI agents a cheap first pass over unfamiliar code before reading full files.
- Return precise file/range metadata so agents can open the smallest useful slice.
- Reuse ast-grep's existing language detection, custom language configuration, ignore
  handling, glob filtering, stdin support, and parallel file walking.
- Use ast-grep rules for extraction logic rather than raw tree-sitter queries.
- Keep symbol kinds LSP-compatible.
- Support human-readable text, single-file JSON, and multi-file JSONL.
- Keep output bounded for model context.

## Non-Goals

- This command is not a replacement for `run` or `scan`.
- This command does not perform rewriting, linting, or rule evaluation.
- This command does not provide full semantic resolution.
- This command does not answer call graph or type-resolution questions.
- The initial implementation does not need perfect import/export semantics for every
  language.

## Ideal Interface

```sh
sg outline <query> [OPTIONS] [PATHS]...
```

Queries:

```text
map         Return a compact structural map of files.
imports     Return import/dependency edges.
exports     Return public/exported API symbols.
members     Return children of a container symbol.
```

Recommended output defaults:

```text
Single file query      json
Multi-file query       jsonl
Human terminal         text
```

Agents should usually request `--format jsonl` for directory scans and `--format json`
for scoped single-file questions.

## Common Options

These options should be shared across queries where applicable. Filters such as
`--name`, `--name-regex`, `--kind`, and `--role` refine results returned by an active
query; they are not a separate symbol lookup command.

```text
--format <text|json|jsonl>
--budget <N>             Approximate result budget for model context.
--max-items <N>          Hard maximum record count.
--lang <LANG>            Parse input as a specific language.
--stdin                  Read source from stdin. Requires --lang.
--name <NAME>            Exact symbol/import/export name.
--name-regex <REGEX>     Regex symbol/import/export name.
--kind <SYMBOL_KIND>     LSP SymbolKind filter. Repeatable.
--role <ROLE>            definition, import, or export. Repeatable.
--flat                   Emit independent records.
--depth <N>              Maximum nesting depth for tree output.
--signature              Include declaration/signature snippets.
--no-snippet             Exclude all source snippets.
--outline-rules <FILE>   Load additional outline extractor definitions. Repeatable.
--no-default-outline-rules
                          Disable bundled extractor definitions.
--globs <GLOB>           Reuse ast-grep input filtering.
--follow                 Reuse ast-grep symlink behavior.
--no-ignore <TYPE>       Reuse ast-grep ignore controls.
--threads <NUM>          Reuse ast-grep parallel walk controls.
```

`--budget` is more agent-oriented than `--max-items`: it can later represent an
approximate output byte/token/record budget. An MVP can implement it as item count.

Suggested applicability:

| Option group | Applies to | Intent |
| --- | --- | --- |
| Input and output: `--format`, `--lang`, `--stdin`, `--globs`, ignore/walk options | all queries | Reuse ast-grep's existing input model. |
| Bounds: `--budget`, `--max-items`, `--depth`, `--flat`, `--signature`, `--no-snippet` | mostly `map` and `members`; `--budget`/`--max-items` can apply everywhere | Keep output bounded and choose tree versus flat shape. |
| Symbol filters: `--name`, `--name-regex`, `--kind`, `--role` | `map`, `exports`, and `members` where meaningful | Filter already-extracted outline records. |
| Import filters: `--to`, `--bindings` | `imports` | Filter dependency edges and optionally flatten bindings. |
| Export filters: `--re-exports`, `--definitions-only` | `exports` | Choose whether re-export statements are included. |

## Query Details

### `map`

Purpose: answer "what is in these files?"

```sh
sg outline map crates/cli/src --format jsonl --budget 200
sg outline map crates/cli/src/scan.rs --depth 1 --format json
```

Ideal behavior:

- Summarizes each file's top-level symbols.
- Defaults to shallow depth.
- Includes counts by kind and role.
- Can return one record per file or one record per top-level symbol.

This is the agent's first pass over an unfamiliar area.

### `imports`

Purpose: answer "what does this file depend on?" and "who depends on this module?"

```sh
sg outline imports crates/cli/src/run.rs --format json
sg outline imports crates --to ast-grep-config --format jsonl
sg outline imports crates/cli/src/run.rs --to ast-grep-config --bindings --format json
```

Ideal behavior:

- For a file, lists imported modules and imported bindings.
- Across a directory, acts like a dependency-edge query.
- Emits source path, imported module, imported binding, alias, and range.

Suggested query-specific options:

```text
--to <MODULE>       Filter by imported module/package/path.
--bindings          Flatten import clauses into one row per imported binding.
```

### `exports`

Purpose: answer "what is the visible API?"

```sh
sg outline exports crates/config/src --format jsonl
sg outline exports crates/cli/src/run.rs --format json
```

Ideal behavior:

- Includes exported definitions and re-exports.
- Distinguishes `role: definition` with `exported: true` from `role: export`.
- Can compare public surface before and after edits.

Suggested query-specific options:

```text
--re-exports        Include re-export statements. Enabled by default.
--definitions-only  Exclude re-export statements without local definitions.
```

### `members`

Purpose: answer "what belongs to this class, struct, enum, interface, trait, impl, or
module?"

```sh
sg outline members src/parser.ts --of Parser --kind method --format json
sg outline members crates/core/src/node.rs --of Node --of-kind struct --recursive --format json
```

Ideal behavior:

- Finds descendants of a named container.
- Supports `--of-kind` to disambiguate.
- Supports `--recursive`.
- Returns member ranges without forcing the agent to read the whole container body.

Suggested query-specific options:

```text
--of <SYMBOL_NAME>       Required container name.
--of-kind <SYMBOL_KIND>  Optional container kind disambiguation.
--recursive             Include recursively nested members.
```

## Output Contract

The ideal default machine output is JSONL for multi-file queries and JSON for single-file
queries. Every flat record should be independently useful:

```json
{
  "path": "crates/cli/src/lib.rs",
  "language": "rs",
  "query": "map",
  "symbol": {
    "name": "Commands",
    "kind": 10,
    "kindName": "Enum",
    "role": "definition",
    "exported": false,
    "range": {
      "start": { "line": 49, "column": 1, "byte": 1200 },
      "end": { "line": 68, "column": 2, "byte": 1700 }
    },
    "selectionRange": {
      "start": { "line": 50, "column": 6, "byte": 1210 },
      "end": { "line": 50, "column": 14, "byte": 1218 }
    },
    "container": null,
    "signature": "enum Commands",
    "score": 0.94
  }
}
```

Important properties:

- `path` is always present.
- `range` is always present, so an agent can open a precise slice.
- `kind` uses LSP `SymbolKind`.
- `role` distinguishes definition/import/export.
- `container` is present in flat output as parent-symbol metadata; this is not a
  standalone `container` query.
- `signature` is short and body-free.
- `score` is optional, but useful for broad fuzzy queries.

Grouped JSON can use an LSP-like tree shape for single-file outline output:

```json
{
  "path": "src/parser.ts",
  "language": "ts",
  "items": [
    {
      "name": "Parser",
      "kind": 5,
      "kindName": "Class",
      "role": "definition",
      "exported": true,
      "range": {
        "start": { "line": 40, "column": 1, "byte": 1200 },
        "end": { "line": 98, "column": 2, "byte": 2500 }
      },
      "selectionRange": {
        "start": { "line": 40, "column": 14, "byte": 1213 },
        "end": { "line": 40, "column": 20, "byte": 1219 }
      },
      "signature": "export class Parser",
      "nodeKind": "class_declaration",
      "children": [
        {
          "name": "parse",
          "kind": 6,
          "kindName": "Method",
          "role": "definition"
        }
      ]
    }
  ]
}
```

Text output should remain concise and human-readable:

```text
src/parser.ts
  Module       fs                         1:1 import
  Function     parseRule                  12:1 export
  Class        Parser                     40:1 export
    Method     parse                      44:3 definition
    Method     recover                    73:3 definition
```

The final text column is a display label. It should print `export` when
`exported: true`; otherwise it should print the item's `role`.

## Data Model

Use LSP `SymbolKind` names and numeric values.

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[repr(u8)]
pub enum SymbolKind {
  File = 1,
  Module = 2,
  Namespace = 3,
  Package = 4,
  Class = 5,
  Method = 6,
  Property = 7,
  Field = 8,
  Constructor = 9,
  Enum = 10,
  Interface = 11,
  Function = 12,
  Variable = 13,
  Constant = 14,
  String = 15,
  Number = 16,
  Boolean = 17,
  Array = 18,
  Object = 19,
  Key = 20,
  Null = 21,
  EnumMember = 22,
  Struct = 23,
  Event = 24,
  Operator = 25,
  TypeParameter = 26,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SymbolRole {
  Definition,
  Import,
  Export,
}
```

Internal grouped item:

```rust
pub struct OutlineItem {
  pub name: Option<String>,
  pub kind: SymbolKind,
  pub role: SymbolRole,
  pub range: Range,
  pub selection_range: Range,
  pub signature: Option<String>,
  pub detail: Option<String>,
  pub exported: Option<bool>,
  pub node_kind: String,
  pub children: Vec<OutlineItem>,
}

pub struct OutlineFile {
  pub path: PathBuf,
  pub language: SgLang,
  pub items: Vec<OutlineItem>,
}
```

Flat record for JSONL:

```rust
pub struct OutlineRecord {
  pub path: PathBuf,
  pub language: SgLang,
  pub query: OutlineQuery,
  pub symbol: OutlineFlatSymbol,
}

pub struct OutlineFlatSymbol {
  pub name: Option<String>,
  pub kind: SymbolKind,
  pub role: SymbolRole,
  pub range: Range,
  pub selection_range: Range,
  pub signature: Option<String>,
  pub detail: Option<String>,
  pub exported: Option<bool>,
  pub node_kind: String,
  pub container: Option<OutlineContainer>,
  pub score: Option<f32>,
}

pub struct OutlineContainer {
  pub name: Option<String>,
  pub kind: SymbolKind,
  pub range: Range,
}
```

`range` is the full AST node range. `selection_range` is the range of the symbol name
when available. This mirrors LSP `DocumentSymbol`.

`kind` must remain LSP-compatible. `role` is ast-grep outline metadata that explains
why the symbol appears in the outline. This is needed because imports, exports, and
ordinary definitions can share the same LSP `SymbolKind`.

## Symbol Mapping

Do not introduce custom symbol kinds for imports or exports. Map source constructs to
existing LSP symbol kinds and use `role`/`exported` metadata to preserve import/export
meaning.

| Source construct | SymbolKind |
| --- | --- |
| File-level source unit | `File` |
| ES import, Rust `use`, Python import, Go import | `Module` |
| Module declaration | `Module` |
| Namespace declaration | `Namespace` |
| Package declaration | `Package` |
| Class declaration | `Class` |
| Method declaration | `Method` |
| Object/class property | `Property` |
| Struct/class field | `Field` |
| Constructor | `Constructor` |
| Enum declaration | `Enum` |
| Interface, trait, protocol | `Interface` |
| Free function | `Function` |
| Local or top-level variable | `Variable` |
| Constant declaration | `Constant` |
| Object/map key | `Key` |
| Enum member or variant | `EnumMember` |
| Struct declaration | `Struct` |
| Event declaration | `Event` |
| Operator overload | `Operator` |
| Type parameter or generic parameter | `TypeParameter` |

Exports should be represented as metadata on the symbol. Exported definitions use
`role: "definition"` and `exported: true`; re-export statements can use
`role: "export"`.

For languages with public/private visibility, `exported` can mean "externally visible"
when that is the closest language concept.

## Agent Exploration Scenarios

### Add A New CLI Subcommand

Goal: find where commands are declared, where arguments live, and which files expose CLI
behavior.

```sh
sg outline map crates/cli/src --kind enum --kind struct --kind function --format jsonl
sg outline members crates/cli/src/lib.rs --of Commands --of-kind enum --format json
sg outline imports crates/cli/src/lib.rs --format json
sg outline exports crates/cli/src --name-regex 'Arg|run_' --format jsonl
```

How this helps:

- Finds command enums and argument structs without reading all CLI files.
- Shows whether each command is implemented as a separate module.
- Gives the agent exact ranges for the enum, args, and run functions to inspect next.

### Understand A Large File Before Editing

Goal: decide whether a file is relevant and where to read first.

```sh
sg outline map crates/cli/src/scan.rs --depth 1 --format json
sg outline imports crates/cli/src/scan.rs --format json
sg outline exports crates/cli/src/scan.rs --format json
```

How this helps:

- The symbol list gives the file's table of contents.
- Imports reveal dependencies and likely responsibilities.
- Exports reveal the entry points other modules use.

### Map Where A User-Facing Concept Is Implemented

Goal: map words from a task into candidate symbols.

```sh
rg -n 'config|rule|scan|verify' crates
sg outline map crates/config crates/cli/src --kind struct --kind enum --kind function --format jsonl
sg outline exports crates --name-regex 'Config|Rule|Scan|Verify' --format jsonl
```

How this helps:

- Uses fast text search for vocabulary discovery.
- Uses `map` to convert candidate files/subtrees into structural records.
- Highlights public APIs that are more likely to be integration points.

### Trace Dependency Direction

Goal: learn which files depend on a module or package.

```sh
sg outline imports crates --to ast-grep-config --format jsonl
sg outline imports crates/cli/src --to ast-grep-core --format jsonl
sg outline imports crates/cli/src/run.rs --bindings --format json
```

How this helps:

- Identifies files that use a crate/module.
- With `--bindings`, shows which imported names are used from that dependency.
- Helps decide whether a change belongs near the importer or exported API.

### Inspect Public API Before Refactoring

Goal: avoid breaking externally visible symbols.

```sh
sg outline exports crates/config/src --format jsonl
sg outline exports crates/cli/src/run.rs --format json
sg outline map crates/config/src --kind struct --kind enum --kind function --format jsonl
```

How this helps:

- Shows public structs, enums, functions, and re-exports.
- Gives the agent a before/after surface to compare after edits.
- Helps distinguish internal helpers from symbols that need migration care.

### Inspect Changed Files After Editing

Goal: summarize the structure of files that changed, using git as the source of truth
for what changed.

```sh
git diff --name-only HEAD
sg outline map <changed-files> --format jsonl
sg outline exports <changed-files> --format jsonl
```

How this helps:

- `git diff --name-only` is the trusted, familiar way to find changed files.
- `map` summarizes the current structure of those files without inventing a second diff
  model.
- `exports` answers the concrete verification question agents care about most: whether
  the changed files expose public symbols that may need migration notes or tests.

### Find Methods On A Container

Goal: understand the behavior surface of a class, impl, trait, or interface.

```sh
sg outline members src/parser.ts --of Parser --kind method --format json
sg outline members crates/core/src/node.rs --of Node --of-kind struct --recursive --format json
```

How this helps:

- Lists methods without reading the whole container body.
- `--of-kind` disambiguates same-name types/functions.
- `--recursive` helps with nested classes/modules in languages that use them.

### Locate Data Models

Goal: find structs, enums, interfaces, type aliases, and constants before changing data
flow.

```sh
sg outline map crates --kind struct --kind enum --kind interface --format jsonl
rg -n 'DEFAULT|CONFIG|TIMEOUT' crates
sg outline map crates/config crates/cli/src --kind constant --format jsonl
```

How this helps:

- Surfaces data definitions separately from behavior.
- Helps identify serialization/config structures and their owning modules.
- Reduces time spent scanning implementation functions.

### Find Tests Related To A Change

Goal: locate likely test functions before making or verifying a change.

```sh
rg -n 'test|should|snapshot|verify' crates
sg outline map crates --kind function --globs '*test*' --format jsonl
sg outline imports crates --to tempfile --format jsonl
```

How this helps:

- Uses fast text and path filtering to identify likely test files.
- Maps test functions structurally once candidate files are known.
- Import filtering can locate test files by common test dependencies.
- Gives exact function ranges for focused reads.

### Build A Cheap Repository Index

Goal: create a compact symbol inventory for agent-side ranking.

```sh
sg outline map crates --format jsonl --budget 5000
```

How this helps:

- Produces one independently useful JSON object per symbol or top-level declaration.
- Lets the agent rank candidates by path, kind, name, exported status, and container.
- Avoids loading large source files until a likely target is found.

## Extraction Strategy

Extraction must be data-driven. The command should not have Rust match arms such as
"if language is Rust, match `function_item`". Built-in support is a bundled extractor
catalog, and user/custom-language support is additional extractor YAML loaded by
`--outline-rules`.

An extractor is an ast-grep rule-core object plus outline metadata:

```yaml
extractors:
  - id: rust-function
    language: Rust
    kind: function
    role: definition
    name: field:name
    exported: textPrefix:pub
    rule: { kind: function_item }

  - id: rust-function-pattern
    language: Rust
    kind: function
    role: definition
    name: NAME
    exported: textPrefix:pub
    rule:
      pattern:
        context: fn $NAME($$$ARGS) $$$BODY
        selector: function_item

  - id: ts-re-export
    language: TypeScript
    kind: module
    role: export
    name: text
    exported: always
    rule:
      all:
        - kind: export_statement
        - regex: '^\s*export\s+(\{|\*|type\s+\{)'
```

The `rule`, `constraints`, `utils`, and `transform` fields are the same rule-core fields
ast-grep already uses. Outline does not invent a second query language.

Extractor fields:

```text
id          Stable extractor id for diagnostics.
language    Any `SgLang`: built-in language or registered custom language.
kind        LSP SymbolKind, serialized in camelCase.
role        definition, import, or export.
name        How to resolve the display name.
exported    How to resolve visibility/export metadata.
rule        ast-grep rule object. Required.
```

Supported `name` values:

```text
NAME          Use metavariable `$NAME` captured by the ast-grep rule.
$NAME         Same as `NAME`.
field:name    Use the matched node's tree-sitter field named `name`.
text          Use the matched node text, normalized for imports/exports.
auto          Best-effort fallback for built-ins.
```

Supported `exported` values:

```text
always
never
nameUppercase
textPrefix:<PREFIX>
ancestorKind:<NODE_KIND>
auto
```

This schema is intentionally small. It covers the common cases while keeping custom
language support practical. If a language needs richer extraction, the rule itself
should first capture better metavariables before outline grows language-specific code.

Extractor flow:

1. Parse source with `SgLang::ast_grep`.
2. Load bundled extractors unless `--no-default-outline-rules` is set.
3. Load every user extractor file from `--outline-rules`.
4. Keep extractors whose `language` matches the file language.
5. Compile each extractor's rule through `SerializableRuleCore::get_matcher`.
6. Run every matcher against the parsed AST.
7. Use the matched node as `range`.
8. Resolve `name` from configured metavariable, field, text, or fallback.
9. Use the name node as `selection_range` when available.
10. Set `kind`, `role`, and `exported` from extractor metadata.
11. Sort items by start byte.
12. Deduplicate overlapping matches.
13. Nest child symbols by range containment.
14. Apply query-specific filters before printing.

## Language And Custom Language Support

Language expansion is an extractor-catalog problem, not a CLI-code problem.

Built-in extractors should ship for common languages such as Rust, TypeScript, TSX,
JavaScript, Python, and Go. Adding another built-in language should mean adding
extractor entries and tests. It should not require changing the extraction algorithm.

Custom languages work the same way:

1. Register the custom parser in `sgconfig.yml` through ast-grep's existing
   `customLanguages` support.
2. Write one or more outline extractor entries with `language: <custom-language-name>`.
3. Run outline with `--outline-rules <FILE>`.

Example custom language extractor:

```yaml
extractors:
  - id: mylang-def
    language: mylang
    kind: function
    role: definition
    name: NAME
    exported: never
    rule:
      pattern: def $NAME($$$ARGS) $$$BODY
```

Then:

```sh
sg outline map src --outline-rules mylang-outline.yml --format jsonl
```

If a user wants to completely replace bundled behavior, they can disable defaults:

```sh
sg outline map src \
  --no-default-outline-rules \
  --outline-rules project-outline.yml \
  --format jsonl
```

Unsupported languages should return an empty outline and a successful exit status. A
future verbose mode can report "no outline extractors loaded for language X".

## Runtime Integration

The command should reuse the existing worker architecture from
`crates/cli/src/utils/worker.rs`.

Path mode:

1. Build a walk with `InputArgs`.
2. Infer language with `SgLang::from_path(path)` unless `--lang` is provided.
3. Read source with the same file-size safeguards used by `run` and `scan`.
4. Extract outline items.
5. Apply query logic.
6. Send grouped or flat records to the printer.

Stdin mode:

1. Require `--lang`.
2. Read stdin into a string.
3. Parse with the provided language.
4. Extract outline items.
5. Use `STDIN` as the path.

## Exit Codes

This should behave like a listing/introspection command, not a search command:

| Condition | Exit code |
| --- | --- |
| Command completed, including empty outline | `0` |
| Invalid CLI arguments | clap error |
| Fatal read, parse, or configuration error | `2` |

An empty outline is not a failed search.

## Implementation Path

The ideal north star is implemented as query names:

```text
map, imports, exports, members
```

The first implementation should keep semantics intentionally shallow and structural:

```text
map        outline records
imports    import/dependency records
exports    exported/public records
members    child records for a named container
```

### Phase 1

- Add CLI subcommand and argument parsing.
- Add `map`, `imports`, `exports`, and `members` queries.
- Add `SymbolKind`, `SymbolRole`, `OutlineItem`, `OutlineFile`, and `OutlineRecord`.
- Implement text, JSON, and JSONL printers.
- Implement Rust and TypeScript/JavaScript outline rules.
- Support `--format`, `--kind`, `--name`, `--name-regex`, `--budget`, and
  `--max-items`.
- Support path mode and stdin mode.
- Add focused CLI parsing tests and extractor unit tests.

### Phase 2

- Add Python and Go outline rules.
- Add nesting by containment.
- Add `members --of-kind` disambiguation.
- Add precise `signature` extraction.
- Add import binding flattening.

### Phase 3

- Consider persistent outline indexing for repeated agent queries.
- Add snapshot tests for representative files.

## Rejected Designs

`outline` could grow beyond structural summaries into a broader code-intelligence tool:
symbol lookup, current-scope detection, related-code discovery, and structural diffs.
These are attractive agent workflows, and each suggests an obvious command:

- Symbol lookup: `find`.
- Current-scope lookup from a file position: `container`.
- Next-code discovery from a seed symbol: `related`.
- Structural change detection after edits: `diff`.

The problem is that these commands either overlap with existing tools (`rg`, source
range reads, and `git diff`) or require semantic information that ast-grep outline does
not have. For v1, keep the command focused on reliable primitives and reject the
following expanded designs. The supported query set remains only `map`, `imports`,
`exports`, and `members`.

### `find`: Symbol Lookup

Original intent: `find` would answer "where is this symbol or concept?" without asking
the user or agent to write an ast-grep pattern.

Example shape:

```sh
sg outline find crates --name RunArg --format jsonl
sg outline find crates --kind function --name-regex 'scan|verify|rule' --format jsonl
```

It was meant to be a constrained lookup over outline facts: exact/regex symbol names,
kind filters, role filters, exported status, path, range, container, and signature.

Decision: do not include a standalone `find` query in this iteration.

Failure mode: the useful version of `find` would need to be a comprehensive structural
lookup over top-level definitions, nested members, imports, exports, and parent
containers. A partial version is worse than leaving the job to `rg`, `map`, `members`,
`imports`, and `exports` because it looks precise while silently missing important
cases.

Exploratory testing on TypeScript, Go, Python, Rust, Java, and Swift benchmark repos
showed the same pattern:

- Exact top-level lookup is sometimes useful, but overlaps with `rg` plus `map`.
- Nested method lookup wants container-aware behavior, which is better expressed as
  `members --of <NAME>`.
- Export lookup must agree with `exports`; if it does not, agents will trust the wrong
  answer.
- Import binding lookup needs language-specific binding extraction, not source-line
  regex matching.
- Regex lookup becomes noisy when it searches source snippets instead of symbol names.

For the first iteration, prefer clearer commands:

- Use `rg`, shell `find(1)`, or normal path globbing to discover candidate files and
  names.
- Use `map` to inspect file or subtree structure.
- Use `members` for methods and fields under a known container.
- Use `imports` and `exports` for dependency and public API questions.

A future `find` can be reconsidered only if it is comprehensive enough to answer
"where is this symbol?" without surprising gaps and with better ergonomics than grep.

### `container`: Current Scope Lookup

Original intent: `container` would answer "what symbol contains this source position?"
after another tool points to a concrete location.

Example shape:

```sh
sg outline container crates/cli/src/lib.rs --at 88:12 --format json
```

The intended agent scenario is: "I have a compiler error, test failure, or grep hit at
this line. What function or class am I inside?"

Decision: do not include a standalone `container` query in this iteration. The idea
sounds useful, but in an agent workflow the agent already has a concrete file and line.
The natural next action is to read around that line:

```sh
sed -n 'LINE_START,LINE_ENDp' path/to/file
```

Failure mode: `container` adds a tool call after the agent already has a concrete
location, and the agent still usually needs to read source code afterwards. It only helps
when the first read window is too small and the enclosing symbol is large enough to
matter.

Exploratory testing also showed interface ambiguity:

- Directory mode is misleading because a line/column pair has no global meaning across
  many files.
- Receiver-style methods in languages like Go can live outside the nominal type
  declaration range, so "source position containment" and "logical membership" are not
  the same question.

For the first iteration, agents should use normal reads after concrete locations. If a
future `container` returns, it should probably require exactly one file path and prove it
saves reads in real agent traces.

### `related`: Next-Code Discovery

Original intent: `related` would answer "what should I inspect next?" from a seed symbol
or source position.

Example shape:

```sh
sg outline related crates/cli/src/run.rs --symbol RunArg --format jsonl
```

It was meant to return ranked candidates with reasons such as same-file symbol,
importer, exporter, same-name symbol, nearby test, or sibling public API.

Decision: do not include a standalone `related` query in this iteration. The command name
promises semantic help: "what code is actually related to this symbol?" That is deeper
than a local syntax outline can reliably answer. A useful implementation would need some
combination of:

- module/import resolution
- type resolution
- reference lookup
- call graph or data-flow edges
- interface/trait/protocol implementation lookup
- override/inheritance lookup
- re-export resolution
- test-to-subject mapping

ast-grep outline is strongest at local structural facts: definitions in a file, members
inside a container, imports, exports, and source ranges. Without a semantic graph,
`related` becomes a heuristic ranking layer over text and outline records. That is a bad
failure mode for AI agents because it looks intelligent while returning plausible but
unverified neighbors.

Exploratory testing showed the deeper design issue:

- When `related` finds methods on a named type, `members --of <NAME>` is clearer and
  more controllable.
- When it finds importers or same-name symbols, `imports` plus `rg` makes the evidence
  explicit instead of hiding it behind ranking.
- When it returns same-file neighbors, the result often spends budget on syntax that is
  nearby but not semantically important.
- Position-based `related --at` has the same ambiguity as `container`: a line/column pair
  only makes sense for one concrete file.

The better first-iteration design is to expose honest primitives:

- `map` for structural overview.
- `members` for container children.
- `imports` for dependency edges.
- `exports` for public surface.
- shell `rg`, shell `find(1)`, and normal path globbing for fast vocabulary and path
  discovery.

Future work can add narrower commands only when their contract is precise:

- `importers` for files importing a module/path.
- `exports --name <NAME>` for public surface matching.
- `usages` or `refs` only if backed by real symbol/reference resolution.
- `neighbors` only if explicitly documented as heuristic and low-trust.

### `diff`: Structural Change Detection

Original intent: `diff` would answer "did this edit change structure or public API?"
without requiring the agent to manually compare outlines before and after a change.

Example shape:

```sh
sg outline diff --base main --format json
sg outline diff --base main --exports-only --format json
```

It was meant to compare outline records before and after edits and report added,
removed, renamed, or kind-changed symbols.

Decision: do not include a standalone `diff` query in this iteration. Generic structural
diff is hard to explain and easy to misuse. Agents and humans already trust git for
change detection:

```sh
git diff
git diff --stat
git diff --name-only HEAD
```

The useful outline question after editing is not "what is the AST diff?" It is more
concrete:

- Which files changed?
- What is the current structure of those files?
- Did the changed files expose public symbols that may need tests, docs, or migration
  notes?

That can be composed from git plus the existing outline primitives:

```sh
git diff --name-only HEAD
sg outline map <changed-files> --format jsonl
sg outline exports <changed-files> --format jsonl
```

This avoids inventing a second diff model. A future public API verification command can
be considered if it has a narrow contract, for example:

```sh
sg outline exports --changed --base HEAD
```

But a standalone `outline diff` is too vague for v1.

## Open Questions

- Should `--format json` support pretty/compact variants, or should `outline` keep a
  simple `text/json/jsonl` enum?
- Should `map` include imports by default, or should imports appear only in `imports`?
- Should `exports` mean only explicit exports, or should it include public visibility
  such as Rust `pub` and Go capitalized identifiers?
- Should unsupported languages be silent by default or emit warnings when not writing
  JSON?
- Should `--budget` count records, bytes, or an approximate token budget?
