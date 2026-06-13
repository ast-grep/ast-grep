# Outline Command Design

## Problem

Add a top-level `outline` subcommand to ast-grep:

```sh
sg outline [OPTIONS] [PATHS]...
```

`outline` is a compact code-structure primitive for humans and interactive coding
agents. It parses source files through ast-grep/tree-sitter and answers local
navigation questions:

- What symbols are in this file or subtree?
- What does this file import?
- What does this module export?
- What members belong to this class, struct, enum, interface, function, or module?

The command should stay narrow. It extracts one structural model per file, then projects
that model with item selection, name/type filters, and member presentation options. It
should not grow separate subcommands for import lookup, export lookup, symbol lookup,
container lookup, related-code discovery, or structural diffs.

## Goals

- Give agents a cheap first pass over unfamiliar code before reading full files.
- Return precise file/range metadata so agents can open the smallest useful slice.
- Use ast-grep rules for extraction logic, not raw tree-sitter query captures.
- Reuse ast-grep language detection, custom language configuration, ignore handling,
  glob filtering, stdin support, and parallel file walking.
- Use `SymbolType` as ast-grep's outline category model, with values compatible with
  LSP `SymbolKind` names.
- Use concise text as the default output for interactive use.
- Support `--json` for scripts and agents that need structured entries.

## Non-Goals

- This command is not a replacement for `run` or `scan`.
- This command does not perform rewriting, linting, or rule evaluation.
- This command does not provide semantic resolution, type resolution, references, call
  graph edges, or data-flow edges.
- Import/export semantics may be approximate when syntax alone cannot express a
  language's full module system.

## Conceptual Model

`outline` extracts **entries** from each parsed file and projects them into text or
JSON. A `struct Foo`, `class Parser`, `function parse()`, `import ...`, or class
method can each become an entry. An entry is one structural fact with a name, role,
`SymbolType`, source range, first-line signature, AST kind, and optional metadata.

There are two roles:

```text
item     Top-level file/module structure: declarations, imports, and explicit exports.
member   Direct child structure under an item: fields, methods, constructors, variants,
         and similar members.
```

Items can carry import/export flags. Members can carry publicness:

```text
isImport     Top-level item is a dependency/import edge.
isExported   Top-level item belongs to the file/module public surface.
isPublic     Member is syntactically public/externally usable.
```

This model is deliberately simple. It uses a small set of boolean flags for outline
display instead of a large semantic taxonomy. That keeps the data model general across
languages, keeps rendering decisions easy to reason about, and keeps rule-based
extraction practical for built-in and custom languages.

Flags are independent. For example, Rust `pub use internal_mod as api;` is one item
with both `isImport` and `isExported`. `outline` does not recursively dump arbitrary
AST nodes or build a normalized relationship graph; source-like signatures preserve
syntax such as `extends`, `implements`, Rust `impl`, and protocol conformance.

Important terms:

| Term | Meaning |
| --- | --- |
| Role | Entry placement: `item` or `member`. |
| Item | Top-level entry for file/module structure, including declarations, imports, and explicit exports. |
| Member | Direct child entry under an item, such as a field, method, constructor, variant, or namespace/module child. |
| SymbolType | Outline category, such as `class`, `function`, or `struct`. Values are compatible with LSP `SymbolKind` names. |
| Name | The visible item or member name in the current file, such as a local binding name or exported name. |
| Range | Full AST node range for the entry. |
| AST kind | The underlying tree-sitter node kind, such as `class_declaration` or `function_item`. |

## Public CLI Contract

```sh
sg outline [OPTIONS] [PATHS]...
```

Default behavior:

```sh
sg outline <path>
```

When no path is provided and `--stdin` is not used, `outline` searches the current
directory, matching ast-grep's existing input behavior.

The default output depends on whether the input is a file or a directory:

```text
stdin                         --items auto --view auto  =>  --items structure --view digest
all explicit inputs are files --items auto --view auto  =>  --items structure --view digest
any directory input present   --items auto --view auto  =>  --items exports --view names
```

A file outline is for inspecting one file's internal structure, so it shows local
top-level structure excluding imports, with compact direct member names. A directory
outline is for scanning project structure, so it shows only exported surface items by
default.
If files and directories are mixed in one invocation, `auto` resolves command-wide to the
directory default. Per-path defaults would make the same file render differently
depending on how it was reached.

Users can override either default explicitly with `--items` and `--view`.

### Core Options

```text
--json[=<pretty|compact|stream>]
                          Output structured JSON. Follows ast-grep's existing
                          `--json` flag shape.
--color <auto|always|ansi|never>
                          Control ANSI color in text output. Default: auto.
--items <auto|structure|exports|imports|all>
                          Select top-level items. Default: auto.
--type <TYPE[,TYPE...]>   LSP-compatible symbol type filter.
--match <REGEX>          Regex over useful top-level item fields.
--pub-members             Display only public members.
--view <auto|names|signatures|digest|expanded>
                          Control text presentation. Default: auto.
```

Input and extractor options:

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

The current design intentionally does not include `--limit`. See
[Future Expansion: Bounded Output](#future-expansion-bounded-output).

## CLI Semantics

### Item Selection

`--items` selects top-level items before type/name filtering. It does not filter by
symbol type; constants, variables, types, and functions are included whenever the
active rule catalog extracts them.

```text
--items auto          file or directory default
file default          structure
directory default     exports
--items structure     top-level items where isImport is false
--items exports       top-level items where isExported is true
--items imports       top-level items where isImport is true
--items all           all top-level items, including imports, re-exports, and
                      explicit export declarations without local structure
```

When `--items` is omitted, it behaves as `--items auto` and chooses the file or
directory default from the public CLI contract. Mixed file and directory input uses the
directory default.

`isExported` is syntax-only in this command. It can come from explicit export syntax
(`export`, `pub use`) or language public-surface syntax that a built-in outline rule can
recognize locally (`pub`, exported Go names). It does not follow re-export chains or
resolve module visibility across files.

Examples:

```sh
sg outline src
sg outline src --items imports
sg outline src --items exports
sg outline src --items all
```

### Match And Type Filters

`--match <REGEX>` and `--type <TYPE[,TYPE...]>` filter the current item
selection. Neither option is repeatable.

`--match` is deliberately not a custom DSL. It is a Rust-regex regular expression,
case-sensitive by default. Invalid regexes are CLI errors. The regex is applied only
to useful top-level item fields:

- structure items: symbol name, signature, and first source line.
- import items: symbol name, signature, and first source line.
- export edge items: symbol name, signature, and first source line.

`--type` is a comma-separated OR filter over top-level item symbol types. Accepted
values are the lower-camel `symbolType` names used in JSON, such as `class`,
`enumMember`, and `typeParameter`. It never matches or filters members:

```text
--type class,function       keep class or function items
--type method,field         keep no top-level items, because methods and fields
                            are not top-level item types
--match Parser --type class
                            keep Parser class items
```

When `--match` and `--type` are both present, `--match` first selects
top-level items, then `--type` filters those top-level items. Neither option matches
members. Once a top-level item survives, member output is controlled only by `--view`.

Examples:

```sh
sg outline crates --type struct,enum,interface
sg outline crates --items exports --match 'Config|Rule|Scan|Verify'
sg outline src/parser.ts --match Parser --type class --view expanded
```

### View Presentation

`--view` controls the text projection:

```text
auto        Choose `names` for directory input and `digest` for file/stdin input.
names       One block per file: one grouped name line per top-level symbol type.
signatures  One block per file: one source/signature line per top-level symbol.
digest      `signatures` plus compact direct member name digests. File default.
expanded    `signatures` plus one source/signature line per direct member.
```

When `--view` is omitted, it behaves as `--view auto`: directory input uses `names`,
file/stdin input uses `digest`, and mixed file plus directory input uses `names`.

Structural members include:

- fields and properties
- methods and constructors
- enum variants/cases/members
- interface, trait, type, impl, and extension members
- declarations directly inside modules or namespaces

For JavaScript and TypeScript only, named function declarations inside a function body
are also members of the containing function. Large JS/TS files often use local helper
functions as part of a function's navigable structure. Other function-body locals are
not part of the file outline.

Examples:

```sh
sg outline crates --view names
sg outline src/parser.ts --match Parser --view digest
sg outline src/parser.ts --match Parser --view expanded
sg outline src/checker.ts --match checkTypeRelatedTo --view expanded
```

### Member Publicness

Member publicness is intentionally simpler than a full visibility model. Members can
carry `isPublic: true` when language-specific syntax says the member is part of the
usable surface of its parent. Private members use `isPublic: false` when the extractor
can determine that. Languages or rules without this knowledge may leave `isPublic`
absent.

By default, member views display all extracted members. `digest` should list public
members first inside each member symbol type group when `isPublic` is known, then list
the remaining members. Each bucket keeps source order.

`--pub-members` narrows displayed members in `digest` and `expanded` views:

```text
default         show all extracted members.
--pub-members   show only members where isPublic is true.
```

Members with absent `isPublic` are kept by default and removed by `--pub-members`.
`expanded` should keep source order; when `--pub-members` is present, it filters
members without reordering the survivors.

### Ordering

There is no standalone grouping option. Ordering and grouping are determined by
`--view`, because each view answers a different reading task:

```text
names       group top-level item names by symbol type
signatures  show top-level items in source order
digest      show top-level items in source order; group member names by symbol type,
            with public names first when known
expanded    show top-level items and members in source order
```

The extracted outline model itself preserves source order within each file. Grouped text
views should keep symbol type groups in a stable presentation order. `names` keeps
names inside each group in source order. `digest` keeps member names in source order
inside the public and non-public buckets.

Agents or scripts that need a different grouping strategy should use `--json` and
post-process the structured entries. Text views stay opinionated and optimized for
interactive reading.

### Output Mode

Text is the default output.

```text
default          text
--json           pretty-printed JSON
--json=compact   compact JSON
--json=stream    newline-delimited entries
```

Interactive agents should usually use text. They should request `--json` only when they
need to transform, extract, join, or programmatically compare outline entries.

`--view` affects text output only. JSON output always emits the selected structured
model after `--items`, `--match`, `--type`, and `--pub-members` filtering.

## Output Contract

### Text Output

Text output should prefer compact file/symbol digests, source lines, or names over raw
metadata. It should not print `role`, `isImport`, or `isExported` labels by default.
Color is only a reading aid:

- `names` colors the symbol type label.
- `signatures`, `digest`, and `expanded` color the entry name inside each signature line.
- exported top-level item names are bold in `digest` and `expanded`, but not in
  `signatures`.
- member digest lines are indented; plural labels like `fields:` use the member
  symbol type color.
- private members are dimmed; their name keeps the member symbol type color.

With `--view names`:

```text
src/parser.ts
class: Parser
function: parseRule, parsePattern
```

With `--view signatures`:

```text
src/parser.ts
12: export function parseRule(...)
40: export class Parser
```

File default `--view digest`:

```text
src/parser.ts
12: export function parseRule(...)
40: export class Parser
    methods: parse, recover
```

With `--view expanded`:

```text
src/parser.ts
12: export function parseRule(...)
40: export class Parser
44:   parse(...)
73:   recover(...)
```

Empty output behavior:

- Direct file and stdin input print an explicit file block when no selected item remains.
- Directory walks suppress files with no selected items.
- If a directory or mixed-input invocation selects nothing overall, print one
  command-level `nothing found` message.

Direct file example:

```text
src/empty.ts
nothing found
```

### JSON Output

`--json` returns grouped file output. `--json=stream` returns one independently useful
entry per line. JSON ranges use one-based line and column numbers, matching text output.

Streamed entry shape:

```json
{
  "path": "crates/cli/src/lib.rs",
  "language": "rs",
  "symbol": {
    "name": "Commands",
    "symbolType": "enum",
    "role": "item",
    "isImport": false,
    "isExported": false,
    "range": {
      "start": { "line": 49, "column": 1, "byte": 1200 },
      "end": { "line": 68, "column": 2, "byte": 1700 }
    },
    "container": null,
    "signature": "enum Commands",
    "astKind": "enum_item"
  }
}
```

Grouped file shape:

```json
{
  "path": "src/parser.ts",
  "language": "ts",
  "items": [
    {
      "name": "Parser",
      "symbolType": "class",
      "role": "item",
      "isImport": false,
      "isExported": true,
      "range": {
        "start": { "line": 40, "column": 1, "byte": 1200 },
        "end": { "line": 98, "column": 2, "byte": 2500 }
      },
      "signature": "export class Parser",
      "astKind": "class_declaration",
      "members": [
        {
          "name": "parse",
          "symbolType": "method",
          "role": "member",
          "isPublic": true,
          "range": {
            "start": { "line": 44, "column": 3, "byte": 1300 },
            "end": { "line": 72, "column": 4, "byte": 1900 }
          },
          "signature": "parse(...)",
          "astKind": "method_definition"
        }
      ]
    }
  ]
}
```

Important properties:

- `path` is always present.
- `range` is always present, so an agent can open a precise slice.
- `symbolType` uses LSP `SymbolKind` names serialized as lower camel case.
- `role` is always `item` or `member`.
- `isImport` and `isExported` are present on top-level items.
- `isPublic` is optional and only meaningful for members; it is absent or null for
  top-level items.
- `container` is present in stream output for parent-symbol metadata.

## Agent Examples

### Understand A Large File Before Editing

```sh
sg outline crates/cli/src/scan.rs
sg outline crates/cli/src/scan.rs --items imports
sg outline crates/cli/src/scan.rs --items exports
```

This gives a table of contents, dependencies, and public entry points before the agent
reads implementation details.

### Add A CLI Subcommand

```sh
sg outline crates/cli/src --type enum,struct,function
sg outline crates/cli/src/lib.rs --match Commands --type enum --view expanded
sg outline crates/cli/src --items exports --match 'Arg|run_'
```

This finds command enums, argument structs, run functions, and public API surfaces
without reading every CLI file.

### Trace Dependency Direction

```sh
sg outline crates --items imports --match ast-grep-config
sg outline crates/cli/src --items imports --match ast-grep-core
```

This identifies files that depend on a module or package. The agent can then decide
whether a change belongs near the importer or exported API.

### Inspect A Matched Parent Symbol

```sh
sg outline src/parser.ts --match Parser --view expanded
sg outline crates/core/src/node.rs --match Node --type struct --view expanded
```

This lists direct members without reading the whole parent body. `--type` disambiguates
the top-level item type; `--view expanded` controls member output.

### Inspect Changed Files After Editing

```sh
git diff --name-only HEAD
sg outline <changed-files>
sg outline <changed-files> --items exports
```

Git remains the source of truth for what changed. `outline` summarizes the current
structure and public surface of those changed files.

### Build A Structured Symbol Inventory

```sh
sg outline crates --json=stream
```

This is the machine-readable mode for ranking candidates by path, symbol type, name,
item flags, and container. It is not the default interactive-agent mode.

## Data Model

Use ast-grep `SymbolType` names in output. The values are compatible with LSP
`SymbolKind` names, but outline does not expose LSP numeric values.

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SymbolType {
  File,
  Module,
  Namespace,
  Package,
  Class,
  Method,
  Property,
  Field,
  Constructor,
  Enum,
  Interface,
  Function,
  Variable,
  Constant,
  String,
  Number,
  Boolean,
  Array,
  Object,
  Key,
  Null,
  EnumMember,
  Struct,
  Event,
  Operator,
  TypeParameter,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum OutlineRole {
  Item,
  Member,
}
```

Grouped item:

```rust
pub struct OutlineItem {
  pub name: Option<String>,
  pub symbol_type: SymbolType,
  pub role: OutlineRole,
  pub is_import: bool,
  pub is_exported: bool,
  pub range: Range,
  pub signature: Option<String>,
  pub detail: Option<String>,
  pub ast_kind: String,
  pub members: Vec<OutlineMember>,
}

pub struct OutlineMember {
  pub name: Option<String>,
  pub symbol_type: SymbolType,
  pub role: OutlineRole,
  pub is_public: Option<bool>,
  pub range: Range,
  pub signature: Option<String>,
  pub detail: Option<String>,
  pub ast_kind: String,
}

pub struct OutlineFile {
  pub path: PathBuf,
  pub language: SgLang,
  pub items: Vec<OutlineItem>,
}
```

Streamed entry:

```rust
pub struct OutlineEntry {
  pub path: PathBuf,
  pub language: SgLang,
  pub symbol: OutlineFlatSymbol,
}

pub struct OutlineFlatSymbol {
  pub name: Option<String>,
  pub symbol_type: SymbolType,
  pub role: OutlineRole,
  pub is_import: Option<bool>,
  pub is_exported: Option<bool>,
  pub is_public: Option<bool>,
  pub range: Range,
  pub signature: Option<String>,
  pub detail: Option<String>,
  pub ast_kind: String,
  pub container: Option<OutlineContainer>,
}

pub struct OutlineContainer {
  pub name: Option<String>,
  pub symbol_type: SymbolType,
  pub range: Range,
}
```

### Item Flags

`role` is outline placement: `item` for top-level items and `member` for direct children.
Import and export semantics are represented with item flags. Publicness is member-only.

```rust
pub struct Foo {}
```

This is one entry:

```json
{
  "name": "Foo",
  "symbolType": "struct",
  "role": "item",
  "isImport": false,
  "isExported": true
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
  "symbolType": "module",
  "role": "item",
  "isImport": true,
  "isExported": true
}
```

Language accessibility syntax can affect extraction-time metadata, especially member
`isPublic` and whether a top-level item receives `isExported`.

### Symbol Mapping

Do not introduce custom symbol types for imports or exports. Map source constructs to
existing LSP symbol kinds and use `isImport` and `isExported` metadata to preserve
import/export meaning.

| Source construct | `symbolType` |
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
| Operator overload | `Operator` |
| Type parameter or generic parameter | `TypeParameter` |

## Extraction Rules

The CLI contract depends on a data-driven extraction layer, but the rule catalog
schema and language-expansion strategy are documented separately in
[outline-rule-extraction.md](outline-rule-extraction.md). In short, ast-grep rules
select candidate syntax, and outline-specific extraction derives names, signatures,
entry/member placement, member publicness, import/export flags, and direct members from
those matches.

## Runtime And Exit Codes

The command should reuse the existing worker architecture from
`crates/cli/src/utils/worker.rs`.

Path mode:

1. Build a walk with `InputArgs`.
2. Infer language with `SgLang::from_path(path)` unless `--lang` is provided.
3. Read source with the same file-size safeguards used by `run` and `scan`.
4. Extract outline entries.
5. Apply item selection and filters.
6. Print text, grouped JSON, or streamed entries.

Stdin mode:

1. Require `--lang`.
2. Read stdin into a string.
3. Parse with the provided language.
4. Extract outline entries.
5. Use `STDIN` as the path.

Exit codes:

| Condition | Exit code |
| --- | --- |
| Command completed, including empty outline | `0` |
| Invalid CLI arguments | clap error |
| Fatal read, parse, or configuration error | `2` |

An empty outline is not a failed search.

## Future Expansion: Bounded Output

The current design does not expose a built-in output limit. Agents can use shell
composition when they need presentation-level truncation:

```sh
sg outline crates/cli/src | head -n 120
sg outline crates/cli/src --items exports | head -n 80
sg outline crates/cli/src --json=stream | jq 'select(.symbol.symbolType == "function")'
```

A future built-in limit may still be valuable as a safety guard against accidentally
emitting a large subtree. If added, it should not be described as "maximum entries or
tree nodes" because that is ambiguous across text views, pretty JSON, compact JSON, and
streamed entries.

Likely contract:

```text
--limit <N>    Maximum selected top-level items to emit.
```

Possible semantics:

- Count selected top-level items after `--items`, `--type`, and `--match`.
- Do not count member names in `--view digest`.
- Do not count member rows in `--view expanded`.
- Do not split a selected top-level item from its direct members.
- Apply in deterministic file/path/source order.

Leave this out until there is evidence that agents or users need the guardrail
frequently.

## Rejected Designs

`outline` could grow beyond structural summaries into symbol lookup, current-scope
detection, related-code discovery, and structural diffs. These workflows are useful, but
they either overlap with existing tools or require semantic information that outline
does not have.

### `find`

Intent: answer "where is this symbol or concept?" without asking the user or agent to
write an ast-grep pattern.

Example shape:

```sh
sg outline find crates --match RunArg
```

Decision: do not include it. A useful `find` must be comprehensive across top-level
items, direct members, imports, exports, and containers. A partial version looks
precise while silently missing important cases. Use `rg` and then `sg outline` on
candidate files or subtrees.

### `container`

Intent: answer "what symbol contains this source position?" after another tool points to
a concrete location.

Example shape:

```sh
sg outline container crates/cli/src/lib.rs --at 88:12
```

Decision: do not include it. In the agent workflow, the agent already has a concrete
file and line, and usually still needs to read source afterwards. It also confuses source
containment with logical membership, such as Go receiver methods that live outside a
type declaration range.

### `related`

Intent: answer "what should I inspect next?" from a seed symbol or source position.

Example shape:

```sh
sg outline related crates/cli/src/run.rs --symbol RunArg
```

Decision: do not include it. The name promises semantic help that local syntax outline
cannot reliably provide: references, module resolution, call graph edges, inheritance,
test mapping, and re-export resolution. Use `--items imports`, `--items exports`,
`--match <REGEX> --view expanded`, `rg`, and normal path discovery instead.

### `diff`

Intent: answer "did this edit change structure or public API?"

Example shape:

```sh
sg outline diff --base main --items exports
```

Decision: do not include it. Generic structural diff is hard to explain and easy to
misuse. Use git for changed files and outline for the current structure:

```sh
git diff --name-only HEAD
sg outline <changed-files>
sg outline <changed-files> --items exports
```

## Open Questions

- Should unsupported languages be silent by default or emit warnings when not writing
  JSON?
