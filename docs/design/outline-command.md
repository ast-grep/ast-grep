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
that model with role filters, name/type filters, and member presentation options. It
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
- Support `--json` for scripts and agents that need structured records.

## Non-Goals

- This command is not a replacement for `run` or `scan`.
- This command does not perform rewriting, linting, or rule evaluation.
- This command does not provide semantic resolution, type resolution, references, call
  graph edges, or data-flow edges.
- Import/export semantics may be approximate when syntax alone cannot express a
  language's full module system.

## Glossary

| Term | Meaning |
| --- | --- |
| Record | One extracted outline fact: name, symbol type, roles, range, signature, and optional children. |
| Role | A facet that explains which question a record answers: `definition`, `import`, or `export`. Roles are not mutually exclusive. |
| Symbol type | Outline category, such as `class`, `function`, or `struct`. Values are compatible with LSP `SymbolKind` names. |
| AST kind | The underlying tree-sitter node kind, such as `class_declaration` or `function_item`. |
| Anchor | A selected top-level record after `--role`, `--type`, and `--match` filters. Member output is attached to anchors. |
| Member | A direct structural child of an anchor, such as a field, method, constructor, enum variant, or direct namespace/module declaration. |
| Range | Full AST node range for the record. |
| Member digest | Grouped member names rendered on one compact line, such as `method: parse, recover`. |

## Public CLI Contract

```sh
sg outline [OPTIONS] [PATHS]...
```

Default behavior:

```sh
sg outline <path>
```

The default output depends on whether the input is a file or a directory:

```text
stdin                         --role auto --view auto  =>  --role definition --role export --view digest
all explicit inputs are files --role auto --view auto  =>  --role definition --role export --view digest
any directory input present   --role auto --view auto  =>  --role export --view names
```

A file outline is for inspecting one file's internal structure, so it shows local
definitions and exported records with compact direct member names. A directory outline is
for scanning project structure, so it shows only exported surface names by default.
If files and directories are mixed in one invocation, `auto` resolves command-wide to the
directory default. Per-path defaults would make the same file render differently
depending on how it was reached.

Users can override either default explicitly with `--role` and `--view`.

### Core Options

```text
--json[=<pretty|compact|stream>]
                          Output structured JSON. Follows ast-grep's existing
                          `--json` flag shape.
--role <auto|definition|import|export|any[,..]>
                          Select records by role. Repeatable. Default: auto.
--type <TYPE[,TYPE...]>   LSP-compatible symbol type filter.
--match <REGEX>          Regex over role-relevant fields. Repeatable.
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

### Role Selection

Every record has one or more roles:

```text
definition    Local declaration or implementation.
import        Dependency edge.
export        Public/exported surface.
```

`--role` filters records by role membership:

```text
--role auto                          file or directory default
file default                         local definitions or exported records
directory default                    exported/public records
--role definition                    local definitions
--role import                        imports and dependency edges
--role export                        exported/public records
--role definition,export             exported records implemented locally
--role import,export                 exports forwarded from another module
--role definition --role import      local definitions or imports
--role any                           no role filtering
```

Comma-separated roles inside one `--role` are ANDed because roles are facets on one
record. Repeated `--role` flags are ORed. `auto` and `any` are selector modes, not
record roles, and should not be combined with other role filters. When `--role` is
omitted, it behaves as `--role auto` and chooses the file or directory default from the
public CLI contract. Mixed file and directory input uses the directory default.

Examples:

```sh
sg outline src
sg outline src --role import
sg outline src --role export
sg outline src --role definition,export
sg outline src --role import,export
sg outline src --role definition --role import
```

### Match And Type Filters

`--match <REGEX>` and `--type <TYPE>` select anchors inside the current role selection.

`--match` is deliberately not a custom DSL. It is a regular expression, like ripgrep's
pattern argument, applied to useful fields:

- definitions: symbol name, source line, signature, and container name.
- imports: imported target, binding name, alias, and source line.
- exports: exported name, re-export target, alias, source line, and container name.

Filter composition:

```text
--type values separated by comma     OR
repeated --match                     OR
different filter types               AND
```

Members attached by `--view digest` or `--view expanded` do not need to match the
filters. They are preserved because they explain the matched anchor.

Examples:

```sh
sg outline crates --type struct,enum,interface
sg outline crates --role export --match 'Config|Rule|Scan|Verify'
sg outline src/parser.ts --match Parser --type class --view expanded
```

### View Presentation

`outline` is a file-level structure command, not a generic AST-depth command. It exposes
top-level declarations and their direct structural members. It does not recursively dump
arbitrary nested blocks.

`--view` controls the text projection:

```text
auto        Choose `names` for directory input and `digest` for file/stdin input.
names       One block per file: one digest line per top-level symbol type.
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

### Output Mode

Text is the default output.

```text
default          text
--json           pretty-printed JSON
--json=compact   compact JSON
--json=stream    newline-delimited records
```

Interactive agents should usually use text. They should request `--json` only when they
need to transform, extract, join, or programmatically compare outline records.

## Output Contract

### Text Output

Text output should prefer compact file/symbol digests, source lines, or names over raw
metadata. It should not print role labels by default.

With `--view names`:

```text
src/parser.ts
class: Parser
function: parseRule, parsePattern
```

With `--view signatures`:

```text
src/parser.ts
function:
12: export function parseRule(...)

class:
40: export class Parser
```

File default `--view digest`:

```text
src/parser.ts
function:
12: export function parseRule(...)

class:
40: export class Parser
  method: parse, recover
```

With `--view expanded`:

```text
src/parser.ts
function:
12: export function parseRule(...)

class:
40: export class Parser
44:   parse(...)
73:   recover(...)
```

Empty direct file input should be explicit:

```text
src/empty.ts
nothing found
```

### JSON Output

`--json` returns grouped file output. `--json=stream` returns one independently useful
record per line.

Streamed record shape:

```json
{
  "path": "crates/cli/src/lib.rs",
  "language": "rs",
  "symbol": {
    "name": "Commands",
    "symbolType": "enum",
    "roles": ["definition"],
    "range": {
      "start": { "line": 49, "column": 1, "byte": 1200 },
      "end": { "line": 68, "column": 2, "byte": 1700 }
    },
    "container": null,
    "signature": "enum Commands",
    "memberDigest": "variant: Run, Scan, Test, New"
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
      "roles": ["definition", "export"],
      "range": {
        "start": { "line": 40, "column": 1, "byte": 1200 },
        "end": { "line": 98, "column": 2, "byte": 2500 }
      },
      "signature": "export class Parser",
      "memberDigest": "method: parse, recover",
      "astKind": "class_declaration",
      "children": [
        {
          "name": "parse",
          "symbolType": "method",
          "roles": ["definition"]
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
- `roles` is a non-empty array.
- `memberDigest` is present when `--view digest` has grouped direct members.
- `container` is present in stream output for parent-symbol metadata.

## Agent Examples

### Understand A Large File Before Editing

```sh
sg outline crates/cli/src/scan.rs
sg outline crates/cli/src/scan.rs --role import
sg outline crates/cli/src/scan.rs --role export
```

This gives a table of contents, dependencies, and public entry points before the agent
reads implementation details.

### Add A CLI Subcommand

```sh
sg outline crates/cli/src --type enum,struct,function
sg outline crates/cli/src/lib.rs --match Commands --type enum --view expanded
sg outline crates/cli/src --role export --match 'Arg|run_'
```

This finds command enums, argument structs, run functions, and public API surfaces
without reading every CLI file.

### Trace Dependency Direction

```sh
sg outline crates --role import --match ast-grep-config
sg outline crates/cli/src --role import --match ast-grep-core
```

This identifies files that depend on a module or package. The agent can then decide
whether a change belongs near the importer or exported API.

### Inspect A Matched Parent Symbol

```sh
sg outline src/parser.ts --match Parser --view expanded
sg outline crates/core/src/node.rs --match Node --type struct --view expanded
```

This lists direct members without reading the whole parent body. `--type` disambiguates
same-name symbols.

### Inspect Changed Files After Editing

```sh
git diff --name-only HEAD
sg outline <changed-files>
sg outline <changed-files> --role export
```

Git remains the source of truth for what changed. `outline` summarizes the current
structure and public surface of those changed files.

### Build A Structured Symbol Inventory

```sh
sg outline crates --json=stream
```

This is the machine-readable mode for ranking candidates by path, symbol type, name, roles, and
container. It is not the default interactive-agent mode.

## Data Model

Use ast-grep `SymbolType` names in output. The values are compatible with LSP
`SymbolKind` names, but outline does not expose LSP numeric values.

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[repr(u8)]
pub enum SymbolType {
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

Grouped item:

```rust
pub struct OutlineItem {
  pub name: Option<String>,
  pub symbol_type: SymbolType,
  pub roles: Vec<SymbolRole>,
  pub range: Range,
  pub signature: Option<String>,
  pub member_digest: Option<String>,
  pub detail: Option<String>,
  pub target: Option<String>,
  pub alias: Option<String>,
  pub ast_kind: String,
  pub children: Vec<OutlineItem>,
}

pub struct OutlineFile {
  pub path: PathBuf,
  pub language: SgLang,
  pub items: Vec<OutlineItem>,
}
```

Streamed record:

```rust
pub struct OutlineRecord {
  pub path: PathBuf,
  pub language: SgLang,
  pub symbol: OutlineFlatSymbol,
}

pub struct OutlineFlatSymbol {
  pub name: Option<String>,
  pub symbol_type: SymbolType,
  pub roles: Vec<SymbolRole>,
  pub range: Range,
  pub signature: Option<String>,
  pub member_digest: Option<String>,
  pub detail: Option<String>,
  pub target: Option<String>,
  pub alias: Option<String>,
  pub ast_kind: String,
  pub container: Option<OutlineContainer>,
}

pub struct OutlineContainer {
  pub name: Option<String>,
  pub symbol_type: SymbolType,
  pub range: Range,
}
```

### Roles Are Facets

Roles are facets, not mutually exclusive categories. One source construct can answer
more than one question.

```rust
pub struct Foo {}
```

This is one record:

```json
{
  "name": "Foo",
  "symbolType": "struct",
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
  "symbolType": "module",
  "roles": ["import", "export"],
  "target": "internal_mod",
  "alias": "api"
}
```

Language accessibility syntax should be used only to decide whether a record receives
the `export` role. Rust `pub`, Go capitalized names, Java `public` top-level
declarations, and Swift `public`/`open` declarations can map to
`roles: ["definition", "export"]` when they are part of the file/module API surface.
Do not expose a separate visibility axis in the CLI.

### Symbol Mapping

Do not introduce custom symbol types for imports or exports. Map source constructs to
existing LSP symbol kinds and use `roles`, `target`, and `alias` metadata to preserve
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

## Extraction Strategy

Extraction must be data-driven. The command should not have Rust match arms such as
"if language is Rust, match `function_item`". Built-in support is a bundled extractor
catalog. User and custom-language support is additional extractor YAML loaded by
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
kind        SymbolType in extractor YAML. Serialized as public `symbolType`.
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

`addRoles` is role-oriented. It should answer "should this source construct also have
the export, import, or definition role?" rather than exposing language visibility as a
separate concept.

Extractor flow:

1. Parse source with `SgLang::ast_grep`.
2. Load bundled extractors unless `--no-default-outline-rules` is set.
3. Load user extractor files from `--outline-rules`.
4. Keep extractors whose `language` matches the file language.
5. Compile each extractor's rule through `SerializableRuleCore::get_matcher`.
6. Run every matcher against the parsed AST.
7. Use the matched node as `range`.
8. Resolve `name` from configured metavariable, field, text, or fallback.
9. Set symbol type, roles, target, and alias, then apply conditional `addRoles`.
10. Sort items by start byte.
11. Deduplicate overlapping matches by range, symbol type, and name. Merge roles instead of
    emitting duplicate records.
12. Nest structural members by range containment or language-specific membership rules.
13. Apply role selection, anchor filters, and member presentation before printing.

## Language And Custom Language Support

Language expansion is an extractor-catalog problem, not a CLI-code problem.

Built-in extractors should ship for common languages such as Rust, TypeScript, TSX,
JavaScript, Python, and Go. Adding another built-in language should mean adding
extractor entries and tests, not changing the extraction algorithm.

Custom languages work the same way:

1. Register the custom parser in `sgconfig.yml` through ast-grep's existing
   `customLanguages` support.
2. Write one or more outline extractor entries with `language: <custom-language-name>`.
3. Run outline with `--outline-rules <FILE>`.

Example:

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

```sh
sg outline src --outline-rules mylang-outline.yml
```

To completely replace bundled behavior:

```sh
sg outline src \
  --no-default-outline-rules \
  --outline-rules project-outline.yml
```

Unsupported languages should return an empty outline and a successful exit status.

## Runtime And Exit Codes

The command should reuse the existing worker architecture from
`crates/cli/src/utils/worker.rs`.

Path mode:

1. Build a walk with `InputArgs`.
2. Infer language with `SgLang::from_path(path)` unless `--lang` is provided.
3. Read source with the same file-size safeguards used by `run` and `scan`.
4. Extract outline items.
5. Apply role selection and filters.
6. Print text, grouped JSON, or streamed records.

Stdin mode:

1. Require `--lang`.
2. Read stdin into a string.
3. Parse with the provided language.
4. Extract outline items.
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
sg outline crates/cli/src --role export | head -n 80
sg outline crates/cli/src --json=stream | jq 'select(.symbol.symbolType == "function")'
```

A future built-in limit may still be valuable as a safety guard against accidentally
emitting a large subtree. If added, it should not be described as "maximum records or
tree items" because that is ambiguous across text views, pretty JSON, compact JSON, and
streamed JSON records.

Likely contract:

```text
--limit <N>    Maximum selected top-level anchors to emit.
```

Possible semantics:

- Count selected top-level anchors after `--role`, `--type`, and `--match`.
- Do not count member names in `--view digest`.
- Do not count member rows in `--view expanded`.
- Do not split a selected anchor from its direct members.
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
definitions, direct members, imports, exports, and containers. A partial version looks
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
test mapping, and re-export resolution. Use `--role import`, `--role export`,
`--match <REGEX> --view expanded`, `rg`, and normal path discovery instead.

### `diff`

Intent: answer "did this edit change structure or public API?"

Example shape:

```sh
sg outline diff --base main --role export
```

Decision: do not include it. Generic structural diff is hard to explain and easy to
misuse. Use git for changed files and outline for the current structure:

```sh
git diff --name-only HEAD
sg outline <changed-files>
sg outline <changed-files> --role export
```

## Open Questions

- Should `--role definition` include top-level constants and variables by default, or
  should the default output prefer named type/function declarations only?
- Which language-specific accessibility rules should assign the `export` role?
- Should unsupported languages be silent by default or emit warnings when not writing
  JSON?
