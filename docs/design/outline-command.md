# Outline Command Design

## Summary

Add a new top-level `outline` subcommand to ast-grep:

```sh
sg outline <query> [OPTIONS] [PATHS]...
```

The command is an AI-agent-friendly structural code intelligence primitive. It parses
source files through ast-grep/tree-sitter and answers navigation questions such as:

- What symbols are in these files?
- Where is this symbol defined?
- What does this file import?
- What does this module export?
- What members belong to this class, struct, enum, interface, or module?
- What symbol contains this source position?
- Did this edit change the visible API surface?

The symbol kind model should follow the Language Server Protocol `SymbolKind` enum so
the output can be consumed by editors and downstream tools without ast-grep-specific
symbol categories.

## Design Principle

AI agents do not primarily need a pretty document outline. They need bounded,
machine-readable answers that help decide which file or range to open next.

The ideal command should therefore be query-oriented, not only display-oriented:

```sh
sg outline map crates/cli/src --format jsonl --budget 200
sg outline find crates --name RunArg --format jsonl
sg outline imports crates --to ast-grep-config --format jsonl
sg outline exports crates/config/src --format jsonl
sg outline members crates/cli/src/lib.rs --of Commands --format json
sg outline container crates/cli/src/lib.rs --at 88:12 --format json
sg outline diff --base main --format json
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
find        Find symbols by name, kind, role, or regex.
imports     Return import/dependency edges.
exports     Return public/exported API symbols.
members     Return children of a container symbol.
container   Return the smallest symbol containing a position or range.
related     Return structurally related symbols, using imports/exports/name proximity.
diff        Compare outlines before and after a change.
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

These options should be shared across queries where applicable:

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
--globs <GLOB>           Reuse ast-grep input filtering.
--follow                 Reuse ast-grep symlink behavior.
--no-ignore <TYPE>       Reuse ast-grep ignore controls.
--threads <NUM>          Reuse ast-grep parallel walk controls.
```

`--budget` is more agent-oriented than `--max-items`: it can later represent an
approximate output byte/token/record budget. An MVP can implement it as item count.

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

### `find`

Purpose: answer "where is this symbol or concept?"

```sh
sg outline find crates --name RunArg --format jsonl
sg outline find crates --kind function --name-regex 'scan|verify|rule' --format jsonl
```

Ideal behavior:

- Supports exact and regex names.
- Supports kind and role filters.
- Returns flat records sorted by relevance.
- Uses path, name, kind, exported status, and container for ranking.

`find` is not a general ast-grep search query. It is a constrained lookup over outline
facts. Compared to `sg run`:

| Capability | `sg run` | `sg outline find` |
| --- | --- | --- |
| Query language | Arbitrary ast-grep pattern/rule | Name, regex, kind, role, visibility |
| Search target | Any AST node | Extracted symbols/imports/exports only |
| Output | Matches | Symbol records with path/range/container/signature |
| Best use | Precise syntax search or rewrite | Code navigation and definition discovery |
| Expressiveness | High | Deliberately constrained |
| Agent ergonomics | Requires knowing syntax shape | Works with symbol names and kinds |

The implementation can use ast-grep rules internally to extract symbols, but users should
not need to write ast-grep patterns for common navigation questions.

`find` is also not a replacement for `rg`. Ripgrep is better for arbitrary text. `find`
is useful when an agent wants typed records like "function named scan", "exported
struct named RuleConfig", or "method parse under Parser", with exact source ranges.

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

### `container`

Purpose: answer "what symbol am I looking at?"

```sh
sg outline container crates/cli/src/lib.rs --at 88:12 --format json
```

Ideal behavior:

- Returns the smallest containing symbol for a position.
- Also returns parent containers.
- Useful after a compiler, test, or grep result points to a line.

Suggested query-specific options:

```text
--at <LINE:COLUMN>
--byte <BYTE_OFFSET>
--range <START_LINE:START_COLUMN-END_LINE:END_COLUMN>
```

### `related`

Purpose: answer "what should I inspect next?"

```sh
sg outline related crates/cli/src/run.rs --symbol RunArg --format jsonl
```

Ideal behavior:

- Uses structural heuristics, not semantic type analysis.
- Can include same-file members, exports, imports, same-name symbols, and nearby tests.
- Returns ranked candidates with reasons.
- Runs with a strict budget and bounded expansion.

This is aspirational. It can be built from cheaper primitives: `find`, `imports`,
`exports`, and naming conventions.

`related` must be more useful than a grep call by returning typed reasons, not just text
matches. Example output reasons:

```text
same-file-container      Symbol is in the same containing class/module.
same-exported-name       Symbol has the same name and is exported elsewhere.
imports-seed-module      File imports the module that defines the seed symbol.
exported-from-module     File re-exports the seed symbol or module.
test-name-match          Test symbol name matches the seed symbol or file stem.
nearby-public-api        Exported symbol is in the same file or sibling module.
```

To avoid being slower than useful:

1. Start from a seed: `--symbol`, `--file`, or `--at`.
2. Extract the seed file outline first.
3. Expand only through cheap structural edges: imports, exports, same-name symbols,
   containers, file/module naming conventions, and test naming conventions.
4. Apply path/glob/language filters before parsing.
5. Use fixed-string prefilters where possible before AST parsing, for example symbol
   name, imported module string, or file stem.
6. Parse files in parallel using the existing worker infrastructure.
7. Stop at `--budget`/`--max-items`.
8. Return ranked records with `reason` and `score`.

`related` should not recursively build a full project graph by default. A good first
implementation can be shallow:

```text
depth 0: seed symbol/file
depth 1: same-file symbols, direct imports, direct exports, exact same-name symbols
depth 2: optional, only with --depth 2 or larger budget
```

This makes `related` complementary to ripgrep:

- `rg RunArg` finds every text occurrence quickly.
- `sg outline related --symbol RunArg` returns a small ranked set of definitions,
  exports, importers, containers, and likely tests with precise ranges.

### `diff`

Purpose: answer "did my edit change structure or public API?"

```sh
sg outline diff --base main --format json
sg outline diff --base main --exports-only --format json
```

Ideal behavior:

- Compares outline records before and after changes.
- Reports added, removed, renamed, or kind-changed symbols.
- Can focus on exported symbols only.

This is valuable for agent verification after edits.

## Output Contract

The ideal default machine output is JSONL for multi-file queries and JSON for single-file
queries. Every flat record should be independently useful:

```json
{
  "path": "crates/cli/src/lib.rs",
  "language": "rs",
  "query": "find",
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
- `container` is present in flat output.
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
sg outline find crates/cli/src --name Commands --format jsonl
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

### Find Where A User-Facing Concept Is Implemented

Goal: map words from a task into candidate symbols.

```sh
sg outline find crates --name-regex 'config|rule|scan|verify' --format jsonl
sg outline exports crates --name-regex 'Config|Rule|Scan|Verify' --format jsonl
```

How this helps:

- Produces candidate files and symbols before full-text search.
- Avoids reading many matches in comments, docs, snapshots, or tests.
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
sg outline find crates/config/src --name RuleConfig --format jsonl
```

How this helps:

- Shows public structs, enums, functions, and re-exports.
- Gives the agent a before/after surface to compare after edits.
- Helps distinguish internal helpers from symbols that need migration care.

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
sg outline find crates --kind struct --kind enum --kind interface --format jsonl
sg outline find crates --kind constant --name-regex 'DEFAULT|CONFIG|TIMEOUT' --format jsonl
```

How this helps:

- Surfaces data definitions separately from behavior.
- Helps identify serialization/config structures and their owning modules.
- Reduces time spent scanning implementation functions.

### Find Tests Related To A Change

Goal: locate likely test functions before making or verifying a change.

```sh
sg outline find crates --kind function --name-regex 'test|should|snapshot|verify' --format jsonl
sg outline imports crates --to tempfile --format jsonl
```

How this helps:

- Finds test functions structurally, not just text mentions.
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

Use ast-grep rules per language instead of raw tree-sitter queries. The command should
still parse files with tree-sitter through ast-grep, but extraction logic should be
expressed with ast-grep's pattern/rule model so the implementation stays aligned with
the rest of the project.

Suggested layout:

```text
crates/cli/src/outline.rs
crates/cli/src/outline/extract.rs
crates/cli/src/outline/printer.rs
crates/cli/src/outline/rules.rs
crates/cli/src/outline/rules/rust.yml
crates/cli/src/outline/rules/typescript.yml
crates/cli/src/outline/rules/javascript.yml
crates/cli/src/outline/rules/python.yml
crates/cli/src/outline/rules/go.yml
```

Each outline rule describes one extractable symbol shape. The rule body should use the
same matching primitives as normal ast-grep rules, plus outline-specific metadata:

```yaml
- id: rust-function
  kind: Function
  role: Definition
  rule:
    pattern: fn $NAME($$$ARGS) $$$BODY
    selector: function_item
  name:
    metaVar: NAME
  signature:
    from: node
    maxLines: 1

- id: rust-struct
  kind: Struct
  role: Definition
  rule:
    pattern: struct $NAME $$$BODY
    selector: struct_item
  name:
    metaVar: NAME

- id: rust-use
  kind: Module
  role: Import
  rule:
    kind: use_declaration
  name:
    from: node
```

Proposed rule schema:

```rust
pub struct OutlineRule {
  pub id: String,
  pub kind: SymbolKind,
  pub role: SymbolRole,
  pub rule: RuleConfig<SgLang>,
  pub name: NameSource,
  pub signature: Option<SignatureSource>,
  pub detail: Option<DetailSource>,
  pub exported: Option<ExportSource>,
  pub children: Option<Vec<String>>,
}

pub enum NameSource {
  MetaVar(String),
  Node,
  Field(String),
}

pub enum SignatureSource {
  Node,
  MetaVar(String),
}

pub enum DetailSource {
  Node,
  MetaVar(String),
}

pub enum ExportSource {
  MetaVarPresent(String),
  HasAncestorRule(String),
  LanguageVisibility,
}
```

Extractor flow:

1. Parse source with `SgLang::ast_grep`.
2. Load built-in outline rules for the file language.
3. Build ast-grep `Rule` matchers from those definitions.
4. Run every outline matcher against the parsed AST.
5. Use the matched node as `range`.
6. Resolve `name` from a meta-variable, matched node text, or language-specific helper.
7. Use the resolved name node as `selection_range` when available.
8. Resolve `signature` from the matched node or configured meta-variable.
9. Set `role` from the matching outline rule.
10. Resolve `exported` from explicit outline rule metadata or language-specific helpers.
11. Sort items by start byte.
12. Deduplicate overlapping matches by preferring the more specific rule.
13. Nest child symbols by range containment when appropriate.
14. Apply query-specific filters before printing.

## Language Support Plan

Start with the languages most likely to cover common code exploration tasks:

### Rust

- `use_declaration` -> `Module`, role `Import`
- `mod_item` -> `Module`
- `function_item` -> `Function`
- associated function in `impl_item` -> `Method`
- `struct_item` -> `Struct`
- `enum_item` -> `Enum`
- enum variants -> `EnumMember`
- `trait_item` -> `Interface`
- `type_item` -> `Interface` or `TypeParameter`, depending on rule metadata
- `const_item` -> `Constant`
- `static_item` -> `Variable`

### TypeScript And JavaScript

- `import_statement` -> `Module`, role `Import`
- `export_statement` -> export metadata or role `Export`
- `function_declaration` -> `Function`
- `method_definition` -> `Method`
- `class_declaration` -> `Class`
- `interface_declaration` -> `Interface`
- `type_alias_declaration` -> `Interface`
- lexical const declaration -> `Constant`
- lexical let/var declaration -> `Variable`
- object pair keys -> `Property` or `Key`

### Python

- `import_statement` -> `Module`, role `Import`
- `import_from_statement` -> `Module`, role `Import`
- `function_definition` -> `Function`
- function under class -> `Method`
- `class_definition` -> `Class`
- assignment with uppercase name -> `Constant`
- other assignment -> `Variable`

### Go

- `package_clause` -> `Package`
- `import_declaration` -> `Module`, role `Import`
- `function_declaration` -> `Function`
- `method_declaration` -> `Method`
- `type_declaration` with struct type -> `Struct`
- `type_declaration` with interface type -> `Interface`
- other type declaration -> `Interface`
- `const_declaration` -> `Constant`
- `var_declaration` -> `Variable`

Unsupported languages should return an empty outline and a successful exit status. A
future verbose mode can report unsupported languages explicitly.

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
map, find, imports, exports, members, container, related, diff
```

The first implementation should keep semantics intentionally shallow and structural:

```text
map        outline records
find       symbol lookup over outline records
imports    import/dependency records
exports    exported/public records
members    child records for a named container
container  smallest symbol containing a source position
related    bounded structural proximity, not a full graph
diff       outline record comparison against a git base
```

### Phase 1

- Add CLI subcommand and argument parsing.
- Add `map`, `find`, `imports`, `exports`, `members`, `container`, `related`, and
  `diff` queries.
- Add `SymbolKind`, `SymbolRole`, `OutlineItem`, `OutlineFile`, and `OutlineRecord`.
- Implement text, JSON, and JSONL printers.
- Implement Rust and TypeScript/JavaScript outline rules.
- Support `--format`, `--kind`, `--name`, `--name-regex`, `--budget`, and
  `--max-items`.
- Support `container --at`, `related --symbol`, and `diff --base`.
- Support path mode and stdin mode.
- Add focused CLI parsing tests and extractor unit tests.

### Phase 2

- Add Python and Go outline rules.
- Add nesting by containment.
- Add `members --of-kind` disambiguation.
- Add precise `signature` extraction.
- Add import binding flattening.
- Add more precise `related` reasons and scores.

### Phase 3

- Add richer `diff` detection for renames and moved symbols.
- Add `container --byte` and `container --range`.
- Consider persistent outline indexing for repeated agent queries.
- Add snapshot tests for representative files.

## Open Questions

- Should `--format json` support pretty/compact variants, or should `outline` keep a
  simple `text/json/jsonl` enum?
- Should `map` include imports by default, or should imports appear only in `imports`?
- Should `exports` mean only explicit exports, or should it include public visibility
  such as Rust `pub` and Go capitalized identifiers?
- Should unsupported languages be silent by default or emit warnings when not writing
  JSON?
- Should `--budget` count records, bytes, or an approximate token budget?
