# Outline Rule Extraction Design

## Purpose

This document designs the extraction layer behind `sg outline`. It is a
companion to [outline-command.md](outline-command.md), which describes the CLI
contract and output views.

The extraction layer answers a narrower question:

> Given one parsed source file and a set of ast-grep outline rules, what
> structural entries should `sg outline` produce before CLI filtering and
> rendering?

The central design constraint is that outline extraction must be data-driven and
ast-grep-native. Built-in language support should be a bundled rule catalog, not
hard-coded Rust match arms. Custom-language support should be possible by loading
additional outline rules.

## Design Principles

1. Extraction must be rule-based and configurable.

   Outline extraction uses ast-grep rules to select syntax. Built-in languages
   are supported by bundled rules, and custom languages are supported by loading
   user-provided outline rules. Adding a language should primarily mean adding
   rule definitions and tests, not adding language-specific branches to the CLI.

2. Extraction is syntax-only.

   Outline can use the parsed AST, source ranges, tree-sitter fields, source
   text, and local syntactic context. It must not depend on type resolution,
   reference resolution, call graphs, data flow, build-system analysis, or
   cross-file semantic indexing. Features that require those capabilities belong
   outside the initial outline extractor.

## Current Requirements

The extractor needs to produce enough structure for file-level code
understanding:

- top-level items: declarations, imports, and explicit export-surface syntax.
- members: direct structural children of top-level items.
- import flags: whether a top-level item is a dependency edge.
- export flags: whether a top-level item belongs to the file/module public
  surface.
- member publicness: whether a direct member is syntactically public/externally
  usable when the language has the concept.
- signatures: the first non-empty source line of the matched item/member node.
- names: stable display names for declarations and import/export edges.
- ranges: full AST node ranges so an agent can open the exact source slice.

The extractor does not need to solve semantic code intelligence:

- no type resolution.
- no reference resolution.
- no call graph.
- no normalized inheritance, extension, implementation, or protocol relationship
  model.
- no import path resolution beyond syntax.
- no transitive public-surface resolver in the initial extraction layer.
- no generic "related code" search.
- no custom metadata extraction for ad hoc facts such as `async`, `override`,
  `static`, `abstract`, or language-specific modifiers. Use normal ast-grep
  rules for those queries.

## Output Model Summary

The command contract, CLI filters, JSON shape, and text views are defined in
[outline-command.md](outline-command.md). The extraction layer only needs to produce
that model before CLI filtering and rendering:

```text
role        item or member.
name        visible item/member name.
symbolType  LSP-compatible outline category.
range       full source range of the matched syntax.
signature   first non-empty source line of the matched syntax.
astKind     tree-sitter node kind of the matched syntax.
```

Top-level `item` entries can additionally carry `isImport`, `isExported`, `target`,
and `alias`. Direct `member` entries can additionally carry `isPublic`.

These fields are output facts, not necessarily separate extractor types. For example,
`pub use internal_mod as api;` is one top-level item with both `isImport` and
`isExported`; `pub struct Foo {}` is one top-level item with `isExported`.

### Preserve Syntactic Organization

This section defines what the extractor should treat as parent/child structure.
`outline` should mirror how code is nested or kept flat in the source file, not
regroup declarations into a semantic view.

Two rules follow from this.

Flat source layout stays flat. `impl Foo for A {}` is its own item, and Go receiver
methods stay top-level method/function items unless the syntax nests them.

Members are direct syntactic children of the item that contains them. A class can have
field, method, and constructor members; a module, namespace, enum, or Rust `impl` can
have direct child declarations as members.

The initial model should not recursively expose arbitrary nested blocks, closures,
local variables, expressions, or semantic "related code" that is not organized as a
direct child in the source.

## Extraction Strategy

Extraction must be data-driven. The command should not have Rust match arms such
as "if language is Rust, match `function_item`". Built-in support is a bundled
extractor catalog. User and custom-language support is additional extractor YAML
loaded by `--outline-rules`.

### Rule File Format

An outline rule file is a stream of YAML documents. Each document is one
extractor. This keeps built-in and custom rules appendable, copyable, and close
to ast-grep's existing rule-file style:

```yaml
id: rust-struct
language: Rust
role: item
symbolType: struct
rule:
  pattern: $VIS struct $NAME { $$$BODY }
name: $NAME
isExported:
  has:
    regex: '^pub\b'
---
id: rust-field
language: Rust
role: member
parentRuleIds: [rust-struct]
symbolType: field
rule:
  pattern: $VIS $NAME: $TYPE
name: $NAME
isPublic:
  has:
    regex: '^pub\b'
```

`rule`, `constraints`, `utils`, and `transform` are the same ast-grep rule-core
fields used by existing ast-grep YAML. Outline adds only metadata and output
field mapping.

### Extractor Schema

Each extractor document has this shape:

```text
id          Stable extractor id for diagnostics.
language    Any `SgLang`: built-in language or registered custom language.
role        Output role: `item` or `member`.
parentRuleIds
            For `role: member`, eligible parent item extractor ids.
symbolType  Output symbol type.
rule        ast-grep rule object that selects the candidate node.
constraints ast-grep constraints for metavariables.
utils       ast-grep local utility rules.
transform   ast-grep transformations and rewriters.
name        Output name from a metavar/template. Required.
signature   Optional output signature from a metavar/template.
target      Optional import/export target from a metavar/template.
alias       Optional renamed-from symbol from a metavar/template.
detail      Optional small display detail from a metavar/template.
isImport    Boolean or predicate for top-level import/dependency items.
isExported  Boolean or predicate for top-level public/module-surface items.
isPublic    Boolean or predicate for member publicness.
```

`role`, `symbolType`, and `name` are required. `parentRuleIds` is required for
`role: member` and ignored for `role: item`. `signature` is optional; when omitted,
the extractor uses the first non-empty line of the matched node as the signature.
`target`, `alias`, `detail`, `isImport`, `isExported`, and `isPublic` are omitted
unless the extractor can derive them.

### Text Field Extraction

Text field extraction should reuse ast-grep metavariables, transformations, and
template replacement.

```yaml
id: ts-function
language: TypeScript
role: item
symbolType: function
rule:
  pattern: function $NAME($$$PARAMS) { $$$BODY }
name: $NAME
signature: function $NAME($$$PARAMS)
```

Transforms can feed text fields:

```yaml
id: rust-raw-ident-struct
language: Rust
role: item
symbolType: struct
rule:
  pattern: struct $RAW_NAME { $$$BODY }
transform:
  NAME:
    replace:
      source: $RAW_NAME
      replace: '^r#'
      by: ''
name: $NAME
```

Text field values are interpreted like ast-grep template strings:

```text
$NAME        captured metavariable text.
$CLEAN_NAME  transformed metavariable text.
literal text mixed with $VARS.
```

The initial supported text fields should stay small: `name`, `signature`,
`target`, `alias`, and `detail`. Arbitrary custom fields are a non-goal.

### Signature Field

Signatures let an agent understand a declaration without reading the body. They
should be source-like, not semantically reconstructed.

An extractor can set `signature` with a template:

```yaml
name: $NAME
signature: function $NAME($$$PARAMS)
```

When `signature` is omitted, the extractor uses the simplest useful fallback:

1. Take the matched item/member node's source text.
2. Use the first non-empty line.
3. Trim leading and trailing whitespace.
4. Render that line as the signature.

This fallback is intentionally imperfect for multiline declarations, but it is
predictable, syntax-only, cheap, and easy to test. Future versions can improve
signature extraction without changing the basic outline entry model. See
[Future: Rich Signatures](#future-rich-signatures).

### Boolean Derivation

Boolean derivation fields can be literal booleans or ast-grep rule predicates
evaluated against the matched candidate node. Normal ast-grep relational rules
such as `has` and `inside` keep their usual meaning relative to that candidate.

```text
boolean omitted by extractor       output field is absent
boolean present and rule matches   output field is true
boolean present and rule misses    output field is false
boolean set to true                output field is true
boolean set to false               output field is false
```

The rule catalog should prefer one extractor plus boolean derivation over
duplicated exported/non-exported extractors. For example, `struct Foo {}` and
`pub struct Foo {}` should use one struct extractor with `isExported`, not two
mostly identical extractors.

### Member Attachment

Member extractors declare where their matches can attach with `parentRuleIds`:

```yaml
id: ts-method
language: TypeScript
role: member
parentRuleIds: [ts-class]
symbolType: method
rule:
  pattern: $NAME($$$PARAMS) { $$$BODY }
name: $NAME
```

`parentRuleIds` references `role: item` extractor IDs, not symbol names, type
names, or `SymbolType` values. Unknown IDs and non-item IDs are configuration
errors. It is an eligibility list. Actual attachment is still based on syntax
containment:

1. Extract all item and member candidates in the file.
2. For each member candidate, find the nearest containing item candidate whose
   extractor id is listed in `parentRuleIds`.
3. Attach the member only when no other extracted item or member candidate lies
   strictly between that item and the member.
4. Drop the member if there is no eligible direct parent.

This preserves source organization and keeps flat layouts flat. Do not infer
membership from names, receiver types, implemented traits, module paths, references, or
type resolution.

### Examples

Rust struct:

```yaml
id: rust-struct
language: Rust
role: item
symbolType: struct
rule:
  pattern: $VIS struct $NAME { $$$BODY }
name: $NAME
isExported:
  has:
    regex: '^pub\b'
```

This handles both entries without duplicating the selector:

```rust
struct Foo {}
pub struct Bar {}
```

```json
{ "role": "item", "name": "Foo" }
{ "role": "item", "name": "Bar", "isExported": true }
```

Rust import/re-export:

```yaml
id: rust-use
language: Rust
role: item
symbolType: module
rule:
  pattern: $VIS use $TARGET;
transform:
  NAME:
    replace:
      source: $TARGET
      replace: '^.*::'
      by: ''
name: $NAME
target: $TARGET
isImport: true
isExported:
  has:
    regex: '^pub\b'
```

This keeps ordinary imports and public imports in one extractor:

```rust
use crate::parser::Parser;
pub use internal_mod as api;
```

```json
{ "role": "item", "name": "Parser", "isImport": true, "target": "crate::parser::Parser" }
{ "role": "item", "name": "api", "isImport": true, "isExported": true, "target": "internal_mod" }
```

Rust member publicness:

```yaml
id: rust-field
language: Rust
role: member
parentRuleIds: [rust-struct]
symbolType: field
rule:
  pattern: $VIS $NAME: $TYPE
name: $NAME
signature: $VIS $NAME: $TYPE
isPublic:
  has:
    regex: '^pub\b'
```

TypeScript class/export class:

```yaml
id: ts-class
language: TypeScript
role: item
symbolType: class
rule:
  any:
    - pattern: class $NAME { $$$BODY }
    - pattern: export class $NAME { $$$BODY }
name: $NAME
isExported:
  any:
    - regex: '^export\b'
    - inside:
        kind: export_statement
```

This keeps one class extractor for both forms:

```ts
class Foo {}
export class Bar {}
```

```json
{ "role": "item", "name": "Foo" }
{ "role": "item", "name": "Bar", "isExported": true }
```

Import and explicit export-list syntax are also top-level items with flags:

```ts
const foo = 1;
export { foo };
export { api as publicApi } from "./api";
export * from "./all";
```

The local `const foo` is a normal item. The `export { foo }` item has
`isExported: true`. The `export { api as publicApi } from "./api"` and
`export * from "./all"` items have both `isImport: true` and `isExported: true`
with `target` and `alias` metadata when syntax provides it.

## What To Avoid

### Avoid Export-Specific Duplicated Item Rules

Bad direction:

```yaml
id: rust-struct
role: item
rule:
  pattern: struct $NAME { $$$BODY }
name: $NAME
---
id: rust-exported-struct
role: item
isExported: true
rule:
  pattern: pub struct $NAME { $$$BODY }
name: $NAME
```

This duplicates the core definition rule and gets worse for every language,
construct, and export variation.

### Avoid A Large New Mini-Language

The outline rule format should not become a second query language with many
custom expression types. ast-grep already has rule syntax. Outline-specific
metadata should stay small and purpose-built.

Prefer a small set of extraction helpers that are hard to express as ast-grep
rules:

- get the first source line from the matched node as a signature.
- attach direct member entries by syntax containment.
- normalize import/export target and alias.

### Avoid IDE-Only Symbols

An IDE outline model with only `kind`, `name`, and selection range is not enough
for agent use. Agents need signatures, line ranges, import/export flags, and
members so they can decide whether to read the source body.

### Avoid Semantic Promises In The Extractor

The extraction layer should not claim to answer:

- "Where is this symbol referenced?"
- "What code is related to this symbol?"
- "What is the full transitive public API?"
- "Which implementation will runtime dispatch call?"

Those require reference resolution, type resolution, or cross-file module
resolution. They can be future layers, but they should not be hidden inside the
basic file extractor.

### Avoid Normalized Relationship Fields

Do not add first-class fields such as `extends`, `implements`, `baseTypes`,
`protocols`, or `implementedFor` to the outline entry model.

Intent: make navigation easier by normalizing relationship syntax across
languages.

Decision: reject this for outline extraction. Relationship syntax is highly
language-specific, and normalizing it makes the extractor more complicated while
still being less faithful than the source. It also creates unclear membership
semantics. For Rust, `struct A {}` and `impl Foo for A {}` are two syntactic
entries, and the `impl` block can have its own direct members. For TypeScript,
`class A extends Base implements C {}` is already understandable when preserved
in the class signature. Keeping the syntactic shape also respects whether the
source author chose a nested or flat organization.

Agents that need to inspect relationship syntax can search for the relevant
construct directly with ast-grep rules, or read the signature/body of the
matched entry. Outline should expose the syntax, not normalize a cross-language
relationship graph.

### Avoid Custom Metadata Extraction

Do not let outline rules define arbitrary output fields such as `async`,
`override`, `static`, `abstract`, `decorators`, or language-specific modifier
sets.

Intent: make outline a general structural metadata extractor.

Decision: reject this for outline extraction. These questions are better served
by ordinary ast-grep rules because the user can express the exact syntax they
care about:

```sh
sg run --pattern 'async $METHOD($$$ARGS) { $$$BODY }' src
sg run --pattern 'override $METHOD($$$ARGS) { $$$BODY }' src
```

Outline should stay focused on file structure: items, import/export flags,
names, ranges, first-line signatures, direct members, member publicness, and
source organization.

## Proposed Extraction Pipeline

1. Parse the source file with ast-grep's language parser.
2. Select applicable extractor definitions by language.
3. Validate extractor references such as `parentRuleIds`.
4. Compile and run each extractor's ast-grep `rule` against the parsed AST.
5. For each match, produce a candidate entry:
   - source range from matched node.
   - AST kind from matched node.
   - symbol type from extractor metadata.
   - outline placement from `role`.
   - name from `name`.
   - signature from `signature`, or the first non-empty line of the matched node.
   - target/alias from `target` and `alias` when applicable.
6. Derive boolean values from syntax:
   - import syntax can set `isImport`.
   - top-level export syntax or language public-surface syntax can set `isExported`.
   - member syntax can set `isPublic`.
7. Deduplicate entries by range, symbol type, name, and edge target.
8. Merge duplicate top-level items by range, symbol type, name, target, and
   alias, preserving `isImport` and `isExported`.
9. Attach direct members by syntax containment and `parentRuleIds`.
10. Sort entries in source order.
11. Pass the file model to CLI filtering and rendering.

## Rule Catalog Implications

Built-in language support should arrive in layers:

1. Item selectors.
2. Name and first-line signature extraction.
3. `isImport` and `isExported` derivation for import/export syntax.
4. Member extraction and member publicness.
5. Language refinements for aliases and import/export shapes.

This means an early built-in rule can be useful without pretending to be final.
For example, a Rust struct selector can first prove rendering and entry merging,
then later add field members and member publicness.

The catalog can keep commented reference rules when they help future language
refinement, but active rules should follow the current model.

## Language And Custom Language Support

Language expansion is an extractor-catalog problem, not a CLI-code problem.

Built-in extractors should ship for common languages such as Rust, TypeScript,
TSX, JavaScript, Python, and Go. Adding another built-in language should mean
adding extractor entries and tests, not changing the extraction algorithm.

Custom languages should work the same way:

1. Register the custom parser in `sgconfig.yml` through ast-grep's existing
   `customLanguages` support.
2. Write one or more outline extractor documents with
   `language: <custom-language-name>`.
3. Run outline with `--outline-rules <FILE>`.

Conceptual example:

```yaml
id: mylang-function
language: mylang
role: item
symbolType: function
rule:
  pattern: def $NAME($$$ARGS) $$$BODY
name: $NAME
signature: def $NAME($$$ARGS)
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

Unsupported languages should return an empty outline and a successful exit
status.

## Future: Rich Signatures

The initial extractor should not solve rich signature extraction. It should use
the first non-empty line of the matched item/member node.

Future versions can make signatures more faithful in two possible ways:

1. Parse signatures into components and assemble them.

   The extractor could capture or derive components such as modifiers, keyword,
   name, generic parameters, parameters, return type, and receiver. The existing
   template/metavariable machinery could then assemble those components in
   `signature` or `detail`. This should stay limited to producing
   signature/detail text, not arbitrary structured metadata fields.

2. Extract a header range with expand-style boundaries.

   The extractor could use a mechanism similar to `expandStart`/`expandEnd`:
   start from the matched item/member node, then compute a smaller header range
   ending before a configured body/value field or captured boundary. ast-grep
   already has the underlying ingredients:

   - `Node::range()` for byte offsets.
   - `Node::text()` for matched source text.
   - `Node::field("name")` and other field lookups for tree-sitter fields.
   - `NodeMatch` metavariables from the ast-grep rule.
   - `Root<StrDoc<_>>::get_text()` for slicing the full source.

   This preserves source spelling and handles multiline declarations better than
   first-line extraction, but it should be designed after the core rule schema is
   stable.

## Appendix: Prior Art

We reviewed three existing outline-oriented projects to understand their data
models and extraction tradeoffs.

### ast-bro

Repository: <https://github.com/aeroxy/ast-bro>

Relevant files reviewed:

- `src/core.rs`
- `src/adapters/rust.rs`
- `src/adapters/typescript.rs`
- `src/surface/rust.rs`
- `src/surface/typescript.rs`
- `src/surface/render.rs`

Model:

- A declaration tree contains `kind`, `name`, `signature`, `bases`, `attrs`,
  docs, visibility, line/byte ranges, native AST kind, modifiers, children, and
  calls.
- Imports are separate `ImportBinding` objects with local binding, module, and
  line.
- Public API is handled by a separate surface resolver, not by the basic file
  outline.
- Signatures are stored as textual declaration strings derived from syntax, so
  output preserves language spelling such as `pub fn`, `impl Trait for Type`,
  generic parameters, return types, and base lists.

What worked well:

- Source-like signatures are much more useful to agents than only symbol names.
- Visibility is stored as metadata, not represented as a separate declaration.
- Rust `impl` methods are regrouped onto the implemented type in ast-bro. That
  is useful for a semantic surface view, but ast-grep outline should keep the
  syntactic `impl` entry instead.
- Public-surface resolution is a separate layer that can follow re-exports and
  glob exports.

Limits for ast-grep:

- The adapter logic is imperative Rust code per language. That does not satisfy
  ast-grep's requirement that extraction logic be expressible as ast-grep rules.
- The normalized `bases`/relationship model is useful in ast-bro, but ast-grep
  should not adopt it in outline extraction.
- The surface resolver is valuable, but it is a later feature because it needs
  cross-file resolution.

### ast-outline

Repository: <https://github.com/ast-outline/ast-outline>

Relevant files reviewed:

- `src/ast_outline/core.py`
- `src/ast_outline/adapters/rust.py`
- `src/ast_outline/adapters/typescript.py`

Model:

- A declaration has `kind`, `name`, `signature`, `bases`, attrs, docs,
  visibility, ranges, and children.
- A parse result carries declarations, imports, conditional import counts, noise
  regions, and import regions.
- Text rendering has separate outline and digest views. Imports are optional and
  separate from declarations.
- Signatures are source slices up to the body/header boundary, then normalized
  for concise output. This keeps type annotations, decorators/attrs where
  selected, return types, inheritance, and language-native declaration keywords.

What worked well:

- Signatures are source-like, not reconstructed semantic models.
- Digest output is a rendering choice, not a different extraction model.
- Visibility is language-specific:
  - Rust `pub` and `pub(...)`.
  - TypeScript accessibility modifiers, `#private`, and naming conventions.
  - Go exported-name casing.
  - Python leading underscore convention.
- Imports are first-class enough to render, but they do not dominate the default
  file outline.

Limits for ast-grep:

- Like ast-bro, extraction is adapter code rather than ast-grep rules.
- The normalized `bases` field should not become an ast-grep outline concept.
- Export handling is mostly syntactic and not a full re-export resolver.

### outline-treesitter-provider.nvim

Repository: <https://github.com/epheien/outline-treesitter-provider.nvim>

Relevant files reviewed:

- `lua/outline/providers/treesitter/aerial/init.lua`
- `lua/outline/providers/treesitter/aerial/extensions.lua`
- `queries/rust/aerial.scm`
- `queries/typescript/aerial.scm`

Model:

- The result is an IDE-style symbol tree: kind, name, ranges, selection range,
  scope, parent, and children.
- Extraction uses tree-sitter query captures.
- There is no first-class signature, import/export flag, target, alias, or
  member visibility model.

What worked well:

- LSP-compatible symbol categories are familiar.
- Range containment is enough for many editor outline trees.
- Language-specific display-name postprocessing is useful, such as Rust impl
  display and Go receiver methods.

Limits for ast-grep:

- Raw tree-sitter queries are not acceptable for ast-grep outline extraction.
- An IDE symbol tree is too weak for agent code understanding because it omits
  signatures and import/export facets.
- Selection ranges are useful for editor jumps, but not necessary for the CLI
  extraction model.

## Open Questions

- What is the smallest concrete rule schema that supports text field extraction,
  boolean derivation objects, and members without becoming a DSL?
- Should import/export edge normalization be configured per language or
  implemented as a small set of built-in normalizers referenced by rules?
- Which import/export edge normalizers should ship as built-ins for common alias
  syntaxes such as TypeScript `export { foo as bar }` and Python `import x as y`?
- Should custom-language rules be allowed to declare member publicness and
  import/export flags, or should custom languages initially support only items,
  names, and signatures?
