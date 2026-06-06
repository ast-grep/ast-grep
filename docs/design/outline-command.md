# Outline Command Design

## Summary

Add a new top-level `outline` subcommand to ast-grep:

```sh
sg outline [OPTIONS] [PATHS]...
```

The command is an AI-agent-friendly structural code intelligence primitive. It parses
source files through ast-grep/tree-sitter and answers navigation questions such as:

- What symbols are in these files?
- What does this file import?
- What does this module export?
- What nested items belong to this class, struct, enum, interface, or module?

For v1, the supported surface is deliberately small: one `outline` command with role
and anchor options. The default role selection is a compact structural map. Imports, exports, and
nested details are projections over the same extracted outline records, not separate
command families. Other useful workflows, such as reviewing changed files after an edit,
should be composed from this primitive and existing tools like `git diff`.

The symbol kind model should follow the Language Server Protocol `SymbolKind` enum so
the output can be consumed by editors and downstream tools without ast-grep-specific
symbol categories.

## Design Principle

AI agents do not primarily need a pretty document outline. They need bounded,
machine-readable answers that help decide which file or range to open next.

The ideal command should therefore expose focused role selections over one structural model:

```sh
sg outline crates/cli/src --format jsonl --limit 200
sg outline crates --role import --match ast-grep-config --format jsonl
sg outline crates/config/src --role export --format jsonl
sg outline crates/cli/src/lib.rs --match Commands --kind enum --depth 2 --format json
```

Each role selection should answer a navigation question directly without creating separate
subcommands for data already present in the outline.

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
sg outline [OPTIONS] [PATHS]...
```

Default behavior:

```text
sg outline <path>    Return a compact structural map of files.
```

Role facets are selected with options:

```text
--role definition    Show local definitions. Default.
--role import        Show import/dependency records.
--role export        Show public/export records.
--role any           Show definitions, imports, exports, and mixed-role records.
```

`--role` selects records whose `roles` contain the requested facet or facets. `--match`
and other filters select anchor items within that role selection. `--depth` controls how
much nested structure below each selected anchor is included.

Recommended output defaults:

```text
Single file input      json
Multi-file input       jsonl
Human terminal         text
```

Agents should usually request `--format jsonl` for directory scans and `--format json`
for scoped single-file questions.

## Options

The public option surface should stay small and composable. The command should not add a
general filter DSL; it should expose simple filters over outline records.

Core options:

```text
--format <text|json|jsonl>
--role <definition|import|export|any[,..]>
                          Select records by role facet. Repeatable. Default: definition.
--kind <KIND[,KIND...]>  LSP SymbolKind filter.
--match <REGEX>          Regex pattern over role-relevant fields. Repeatable.
--depth <N|all>          Maximum nesting depth for tree output.
--limit <N>              Maximum records or tree items to emit.
```

Advanced input and extractor options:

```text
--lang <LANG>            Parse input as a specific language.
--stdin                  Read source from stdin. Requires --lang.
--outline-rules <FILE>   Load additional outline extractor definitions. Repeatable.
--no-default-outline-rules
                          Disable bundled extractor definitions.
--globs <GLOB>           Reuse ast-grep input filtering.
--follow                 Reuse ast-grep symlink behavior.
--no-ignore <TYPE>       Reuse ast-grep ignore controls.
--threads <NUM>          Reuse ast-grep parallel walk controls.
```

`--match` is deliberately not a DSL. It is a regular expression, like ripgrep's
pattern argument, applied to the useful text fields for the current role selection:

- definitions: symbol name, source line, signature, and container name.
- imports: imported target, binding name, alias, and source line.
- exports: exported name, re-export target, alias, source line, and container name.

Simple filters select anchor items. With no anchor filters, top-level items in the
current role selection are anchors. With anchor filters, matching items become anchors.
Different filter types are ANDed together. Comma-separated values inside `--kind` are
ORed. Comma-separated values inside one `--role` are ANDed because roles are facets on
one record: `--role definition,export` means "records that are both local definitions
and exports." Repeating `--role` is ORed across role criteria: `--role definition
--role import` means "definitions or imports." `--role any` means no role filtering and
should not be combined with other role criteria. Repeating `--match` is also ORed.
Descendants included by `--depth` do not need to match the anchor filters; they are
preserved because they explain the matched item.

`--limit` is the only public bounding option. It can start as a record/item count and
later evolve toward byte or token budgeting without exposing a second option.

Suggested applicability:

| Option group | Applies to | Intent |
| --- | --- | --- |
| Role selection: `--role` | all outline records | Choose definitions, imports, exports, mixed-role records, or all records. |
| Anchor filters: `--kind`, `--match` | extracted records | Select anchor items without a query language. |
| Output shape: `--format`, `--depth`, `--limit` | all role selections where meaningful | Keep output bounded and choose tree versus flat machine output. |
| Input and extractor configuration: `--lang`, `--stdin`, `--globs`, ignore/walk options, `--outline-rules` | all role selections | Reuse ast-grep's existing input model and rule catalog. |

## View Details

### Default Structural Map

Purpose: answer "what is in these files?"

```sh
sg outline crates/cli/src --format jsonl --limit 200
sg outline crates/cli/src/scan.rs --depth 1 --format json
```

Ideal behavior:

- Summarizes each file's top-level symbols.
- Defaults to shallow depth.
- Includes counts by kind and role facet.
- Can return one record per file or one record per top-level symbol.

This is the agent's first pass over an unfamiliar area.

### Import Role

Purpose: answer "what does this file depend on?" and "who depends on this module?"

```sh
sg outline crates/cli/src/run.rs --role import --format json
sg outline crates --role import --match ast-grep-config --format jsonl
sg outline crates/cli/src/run.rs --role import --depth 2 --format json
```

Ideal behavior:

- For a file, lists imported modules and imported bindings.
- Across a directory, acts like a dependency-edge view.
- Emits source path, imported module, imported binding, alias, and range.

Import filtering should use `--match`; import bindings should be represented as child
items and shown with `--depth` instead of a separate flattening flag.

### Export Role

Purpose: answer "what is the visible API?"

```sh
sg outline crates/config/src --role export --format jsonl
sg outline crates/cli/src/run.rs --role export --format json
```

Ideal behavior:

- Includes public definitions and re-exports.
- Uses multi-role records so `pub struct Foo {}` is one record with
  `roles: ["definition", "export"]`, and `pub use internal_mod as api` is one record
  with `roles: ["import", "export"]`.
- Can compare public surface before and after edits.

Use `--role definition,export` when the agent only wants locally defined public API:

```sh
sg outline crates/config/src --role definition,export --format jsonl
```

This keeps exported definitions such as `pub struct Foo {}` or `export function foo()`,
but excludes export-only edges such as `export { foo as bar }` and re-exported imports
such as `export { foo as bar } from "./mod"` or `pub use internal::Foo`.

Use `--role import,export` to focus on exports forwarded from another module:

```sh
sg outline crates/config/src --role import,export --format jsonl
```

The broader `--role export` result still includes all export records, and its output
roles/source lines let agents distinguish local definitions, local export edges, and
forwarded exports without needing a negative-role option.

### Role Query Recipes

Common role selections map directly to code-understanding questions:

```sh
sg outline src
```

Lists local definitions. This is the default structural map: "what is implemented
here?"

```sh
sg outline src --role import
```

Lists dependency edges: "what does this code depend on?"

```sh
sg outline src --role export
```

Lists the full public/export surface: local exported definitions, export-only aliases,
and re-exported imports.

```sh
sg outline src --role definition,export
```

Lists public API implemented locally. This helps distinguish ownership from forwarding
modules.

```sh
sg outline src --role import,export
```

Lists public API forwarded from another module, such as `export { Foo } from "./foo"`
or `pub use foo::Foo`.

```sh
sg outline src --role definition --role import
```

Lists local implementation plus dependencies while excluding export-only aliases. This
is useful when an agent wants to understand a file's implementation context before
editing.

```sh
sg outline src --role any --match Auth --depth 1
```

Lists every structural fact around a concept: definitions, imports, exports, mixed-role
records, and shallow children.

### Anchor Expansion

Purpose: answer "what belongs to this class, struct, enum, interface, trait, impl, or
module?"

```sh
sg outline src/parser.ts --match Parser --depth 2 --format json
sg outline crates/core/src/node.rs --match Node --kind struct --depth all --format json
```

Ideal behavior:

- Uses `--match` and `--kind` to select anchor items.
- Uses `--depth` to include descendants of matched anchors.
- Returns nested item ranges without forcing the agent to read the whole parent body.

Suggested options:

```text
--match <TEXT>           Select parent symbols by name or source line.
--kind <KIND[,KIND...]>  Disambiguate same-name symbols.
--depth <N|all>          Include direct children or recursively nested descendants.
```

## Output Contract

The ideal default machine output is JSONL for multi-file inputs and JSON for single-file
inputs. Every flat record should be independently useful:

```json
{
  "path": "crates/cli/src/lib.rs",
  "language": "rs",
  "symbol": {
    "name": "Commands",
    "kind": 10,
    "kindName": "Enum",
    "roles": ["definition"],
    "range": {
      "start": { "line": 49, "column": 1, "byte": 1200 },
      "end": { "line": 68, "column": 2, "byte": 1700 }
    },
    "selectionRange": {
      "start": { "line": 50, "column": 6, "byte": 1210 },
      "end": { "line": 50, "column": 14, "byte": 1218 }
    },
    "container": null,
    "signature": "enum Commands"
  }
}
```

Important properties:

- `path` is always present.
- `range` is always present, so an agent can open a precise slice.
- `kind` uses LSP `SymbolKind`.
- `roles` is a non-empty array of facets such as `definition`, `import`, and `export`.
  A single record can have multiple roles.
- `container` is present in flat output as parent-symbol metadata; this is not a
  standalone `container` command.
- `signature` is short and body-free.

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
      "roles": ["definition", "export"],
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
          "roles": ["definition"]
        }
      ]
    }
  ]
}
```

Text output should remain concise and human-readable:

```text
src/parser.ts
  1: import { readFile } from "fs"
  12: export function parseRule(...)
  40: export class Parser
    44: parse(...)
    73: recover(...)
```

Text output should prefer the source line and indentation over exposing raw role labels.
Machine output carries exact `roles` for filtering.

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
  pub roles: Vec<SymbolRole>,
  pub range: Range,
  pub selection_range: Range,
  pub signature: Option<String>,
  pub detail: Option<String>,
  pub target: Option<String>,
  pub alias: Option<String>,
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
  pub symbol: OutlineFlatSymbol,
}

pub struct OutlineFlatSymbol {
  pub name: Option<String>,
  pub kind: SymbolKind,
  pub roles: Vec<SymbolRole>,
  pub range: Range,
  pub selection_range: Range,
  pub signature: Option<String>,
  pub detail: Option<String>,
  pub target: Option<String>,
  pub alias: Option<String>,
  pub node_kind: String,
  pub container: Option<OutlineContainer>,
}

pub struct OutlineContainer {
  pub name: Option<String>,
  pub kind: SymbolKind,
  pub range: Range,
}
```

`range` is the full AST node range. `selection_range` is the range of the symbol name
when available. This mirrors LSP `DocumentSymbol`.

`kind` must remain LSP-compatible. `roles` is ast-grep outline metadata that explains
which role selections the record belongs to. This is needed because imports, exports,
and ordinary definitions can share the same LSP `SymbolKind`, and because one source
construct can belong to multiple role selections.

## Symbol Mapping

Do not introduce custom symbol kinds for imports or exports. Map source constructs to
existing LSP symbol kinds and use `roles`, `target`, and `alias` metadata to preserve
import/export meaning.

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

Roles are facets, not mutually exclusive categories. A source construct can answer more
than one outline question:

```rust
pub struct Foo {}
```

This should be one record:

```json
{
  "name": "Foo",
  "kindName": "Struct",
  "roles": ["definition", "export"]
}
```

Likewise:

```rust
pub use internal_mod as api;
```

This is both an import/dependency edge and an export edge:

```json
{
  "name": "api",
  "kindName": "Module",
  "roles": ["import", "export"],
  "target": "internal_mod",
  "alias": "api"
}
```

Language accessibility syntax should be used only to decide whether a record receives
the `export` role. For example, Rust `pub`, Go capitalized names, Java `public`
top-level declarations, and Swift `public`/`open` declarations can all map to
`roles: ["definition", "export"]` when they are part of the file/module API surface.
Do not expose a separate visibility axis in the CLI; it makes file-level structure
harder to understand.

## Agent Exploration Scenarios

### Add A New CLI Subcommand

Goal: find where commands are declared, where arguments live, and which files expose CLI
behavior.

```sh
sg outline crates/cli/src --kind enum,struct,function --format jsonl
sg outline crates/cli/src/lib.rs --match Commands --kind enum --depth 2 --format json
sg outline crates/cli/src/lib.rs --role import --format json
sg outline crates/cli/src --role export --match 'Arg|run_' --format jsonl
```

How this helps:

- Finds command enums and argument structs without reading all CLI files.
- Shows whether each command is implemented as a separate module.
- Gives the agent exact ranges for the enum, args, and run functions to inspect next.

### Understand A Large File Before Editing

Goal: decide whether a file is relevant and where to read first.

```sh
sg outline crates/cli/src/scan.rs --depth 1 --format json
sg outline crates/cli/src/scan.rs --role import --format json
sg outline crates/cli/src/scan.rs --role export --format json
```

How this helps:

- The symbol list gives the file's table of contents.
- Imports reveal dependencies and likely responsibilities.
- Exports reveal the entry points other modules use.

### Map Where A User-Facing Concept Is Implemented

Goal: map words from a task into candidate symbols.

```sh
rg -n 'config|rule|scan|verify' crates
sg outline crates/config crates/cli/src --kind struct,enum,function --format jsonl
sg outline crates --role export --match 'Config|Rule|Scan|Verify' --format jsonl
```

How this helps:

- Uses fast text search for vocabulary discovery.
- Uses the default outline output to convert candidate files/subtrees into structural
  records.
- Highlights public APIs that are more likely to be integration points.

### Trace Dependency Direction

Goal: learn which files depend on a module or package.

```sh
sg outline crates --role import --match ast-grep-config --format jsonl
sg outline crates/cli/src --role import --match ast-grep-core --format jsonl
sg outline crates/cli/src/run.rs --role import --depth 2 --format json
```

How this helps:

- Identifies files that use a crate/module.
- With `--depth 2`, shows imported bindings nested under module/package edges.
- Helps decide whether a change belongs near the importer or exported API.

### Inspect Public API Before Refactoring

Goal: avoid breaking externally visible symbols.

```sh
sg outline crates/config/src --role export --format jsonl
sg outline crates/cli/src/run.rs --role export --format json
sg outline crates/config/src --kind struct,enum,function --format jsonl
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
sg outline <changed-files> --format jsonl
sg outline <changed-files> --role export --format jsonl
```

How this helps:

- `git diff --name-only` is the trusted, familiar way to find changed files.
- The default outline output summarizes the current structure of those files without
  inventing a second diff model.
- `--role export` answers the concrete verification question agents care about most:
  whether the changed files expose public symbols that may need migration notes or
  tests.

### Expand A Matched Parent Symbol

Goal: understand the behavior surface of a class, impl, trait, or interface.

```sh
sg outline src/parser.ts --match Parser --depth 2 --format json
sg outline crates/core/src/node.rs --match Node --kind struct --depth all --format json
```

How this helps:

- Lists methods without reading the whole parent body.
- `--kind` disambiguates same-name types/functions.
- `--depth all` helps with nested classes/modules in languages that use them.

### Locate Data Models

Goal: find structs, enums, interfaces, type aliases, and constants before changing data
flow.

```sh
sg outline crates --kind struct,enum,interface --format jsonl
rg -n 'DEFAULT|CONFIG|TIMEOUT' crates
sg outline crates/config crates/cli/src --kind constant --format jsonl
```

How this helps:

- Surfaces data definitions separately from behavior.
- Helps identify serialization/config structures and their owning modules.
- Reduces time spent scanning implementation functions.

### Find Tests Related To A Change

Goal: locate likely test functions before making or verifying a change.

```sh
rg -n 'test|should|snapshot|verify' crates
sg outline crates --kind function --globs '*test*' --format jsonl
sg outline crates --role import --match tempfile --format jsonl
```

How this helps:

- Uses fast text and path filtering to identify likely test files.
- Maps test functions structurally once candidate files are known.
- Import filtering can locate test files by common test dependencies.
- Gives exact function ranges for focused reads.

### Build A Cheap Repository Index

Goal: create a compact symbol inventory for agent-side ranking.

```sh
sg outline crates --format jsonl --limit 5000
```

How this helps:

- Produces one independently useful JSON object per symbol or top-level declaration.
- Lets the agent rank candidates by path, kind, name, roles, and container.
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
    roles: [definition]
    addRoles:
      export: textPrefix:pub
    name: field:name
    rule: { kind: function_item }

  - id: rust-function-pattern
    language: Rust
    kind: function
    roles: [definition]
    addRoles:
      export: textPrefix:pub
    name: NAME
    rule:
      pattern:
        context: fn $NAME($$$ARGS) $$$BODY
        selector: function_item

  - id: rust-pub-use
    language: Rust
    kind: module
    roles: [import, export]
    name: text
    target: text
    rule:
      all:
        - kind: use_declaration
        - regex: '^\s*pub\s+use\b'

  - id: ts-re-export
    language: TypeScript
    kind: module
    roles: [import, export]
    name: text
    target: text
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
roles       Non-empty list containing definition, import, and/or export.
addRoles    Optional conditional roles to add when a source predicate matches.
name        How to resolve the display name.
target      Optional module/package/path target for import/export edges.
alias       Optional local alias for import/export edges.
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

Supported `addRoles` predicates:

```text
nameUppercase
textPrefix:<PREFIX>
textPrefixAny:<A>,<B>
notTextPrefixAny:<A>,<B>
ancestorKind:<NODE_KIND>
auto
```

`addRoles` is intentionally role-oriented. It should answer "does this source construct
belong to the export/import/definition projection?" rather than exposing language
visibility as a separate concept. If a language has nuanced accessibility such as
Rust `pub(crate)` or Swift `internal`, only map it to `export` when it is useful for
file/module API exploration.

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
10. Set `kind`, `roles`, `target`, and `alias` from extractor metadata, then apply
    conditional `addRoles`.
11. Sort items by start byte.
12. Deduplicate overlapping matches. If two extractors identify the same source range,
    kind, and name, merge their roles instead of emitting duplicate records.
13. Nest child symbols by range containment.
14. Apply role selection and anchor filters before printing.

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
    roles: [definition]
    name: NAME
    rule:
      pattern: def $NAME($$$ARGS) $$$BODY
```

Then:

```sh
sg outline src --outline-rules mylang-outline.yml --format jsonl
```

If a user wants to completely replace bundled behavior, they can disable defaults:

```sh
sg outline src \
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
5. Apply role selection and filters.
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

The ideal north star is implemented as one command with role-facet options:

```text
sg outline [--role definition|import|export|any[,..]] [--match <TEXT>] [--depth <N|all>] [PATHS]...
```

The first implementation should keep semantics intentionally shallow and structural.
Extraction produces one outline tree per file. Printing then applies role selection:

```text
default / --role definition     records whose roles contain definition
--role import                   records whose roles contain import
--role export                   records whose roles contain export
--role definition,export        local exported definitions
--role import,export            exports forwarded from another module
--role definition --role import local definitions or imports
--role any                      all definition, import, and export records
--match <TEXT>                  select anchor records in the current role selection
--depth <N|all>                 include descendants of matched anchors
```

### Phase 1

- Add CLI subcommand and argument parsing.
- Add default outline output, `--role` selection, anchor filtering, and depth
  expansion.
- Add `SymbolKind`, `SymbolRole`, `OutlineItem`, `OutlineFile`, and `OutlineRecord`.
- Implement text, JSON, and JSONL printers.
- Implement Rust and TypeScript/JavaScript outline rules.
- Support `--format`, `--role`, `--kind`, `--match`, and `--limit`.
- Support path mode and stdin mode.
- Add focused CLI parsing tests and extractor unit tests.

### Phase 2

- Add Python and Go outline rules.
- Add nesting by containment.
- Make anchor filtering precise enough for common parent-symbol expansion.
- Add precise `signature` extraction.
- Add import binding child items.

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
not have. For v1, keep the command focused on one reliable primitive: extract a file
outline and project it with `--role`, anchor filters, and `--depth`.

### `find`: Symbol Lookup

Original intent: `find` would answer "where is this symbol or concept?" without asking
the user or agent to write an ast-grep pattern.

Example shape:

```sh
sg outline find crates --match RunArg --format jsonl
sg outline find crates --kind function --match 'scan|verify|rule' --format jsonl
```

It was meant to be a constrained lookup over outline facts: symbol-name patterns, kind
filters, role-membership filters, path, range, container, and signature.

Decision: do not include a standalone `find` command in this iteration.

Failure mode: the useful version of `find` would need to be a comprehensive structural
lookup over top-level definitions, nested members, imports, exports, and parent
containers. A partial version is worse than leaving the job to `rg` plus `sg outline`
views because it looks precise while silently missing important cases.

Exploratory testing on TypeScript, Go, Python, Rust, Java, and Swift benchmark repos
showed the same pattern:

- Exact top-level lookup is sometimes useful, but overlaps with `rg` plus default
  outline output.
- Nested method lookup wants anchor expansion, which is better expressed as
  `sg outline --match <NAME> --depth 2`.
- Export lookup must agree with `--role export`; if it does not, agents will trust the
  wrong answer.
- Import binding lookup needs language-specific binding extraction, not source-line
  pattern matching.
- Pattern lookup becomes noisy when it searches source snippets instead of symbol names.

For the first iteration, prefer clearer commands:

- Use `rg`, shell `find(1)`, or normal path globbing to discover candidate files and
  names.
- Use default outline output to inspect file or subtree structure.
- Use `--match <NAME> --depth <N>` for methods and fields under a known parent symbol.
- Use `--role import` and `--role export` for dependency and public API questions.

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

Decision: do not include a standalone `container` command in this iteration. The idea
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

Decision: do not include a standalone `related` command in this iteration. The command name
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

ast-grep outline is strongest at local structural facts: definitions in a file, nested
children under matched anchors, imports, exports, and source ranges. Without a semantic graph,
`related` becomes a heuristic ranking layer over text and outline records. That is a bad
failure mode for AI agents because it looks intelligent while returning plausible but
unverified neighbors.

Exploratory testing showed the deeper design issue:

- When `related` finds methods on a named type, `sg outline --match <NAME> --depth 2`
  is clearer and more controllable.
- When it finds importers or same-name symbols, `--role import` plus `rg` makes the
  evidence explicit instead of hiding it behind ranking.
- When it returns same-file neighbors, the result often spends budget on syntax that is
  nearby but not semantically important.
- Position-based `related --at` has the same ambiguity as `container`: a line/column pair
  only makes sense for one concrete file.

The better first-iteration design is to expose honest primitives:

- default outline output for structural overview.
- `--match <NAME> --depth <N>` for parent-symbol children.
- `--role import` for dependency edges.
- `--role export` for public surface.
- shell `rg`, shell `find(1)`, and normal path globbing for fast vocabulary and path
  discovery.

Future work can add narrower commands only when their contract is precise:

- `importers` for files importing a module/path.
- `--role export --match <NAME>` for public surface matching.
- `usages` or `refs` only if backed by real symbol/reference resolution.
- `neighbors` only if explicitly documented as heuristic and low-trust.

### `diff`: Structural Change Detection

Original intent: `diff` would answer "did this edit change structure or public API?"
without requiring the agent to manually compare outlines before and after a change.

Example shape:

```sh
sg outline diff --base main --format json
sg outline diff --base main --role export --format json
```

It was meant to compare outline records before and after edits and report added,
removed, renamed, or kind-changed symbols.

Decision: do not include a standalone `diff` command in this iteration. Generic structural
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
sg outline <changed-files> --format jsonl
sg outline <changed-files> --role export --format jsonl
```

This avoids inventing a second diff model. A future public API verification command can
be considered if it has a narrow contract, for example:

```sh
sg outline --role export --changed --base HEAD
```

But a standalone `outline diff` is too vague for v1.

## Open Questions

- Should `--format json` support pretty/compact variants, or should `outline` keep a
  simple `text/json/jsonl` enum?
- Should `--role definition` include top-level constants and variables by default, or
  should the default output prefer named type/function declarations only?
- Which language-specific accessibility rules should assign the `export` role, such as
  Rust `pub` and Go capitalized identifiers?
- Should unsupported languages be silent by default or emit warnings when not writing
  JSON?
- Should `--limit` count records, tree items, bytes, or an approximate token budget?
