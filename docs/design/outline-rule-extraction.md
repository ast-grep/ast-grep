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

## Investigation Summary

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

## Signature Extraction

Signatures are important because they let an agent understand a declaration
without reading the body. They should be source-like, not semantically
reconstructed.

For the initial version, signature extraction should use the simplest useful
approach:

1. Take the matched item/member node's source text.
2. Use the first non-empty line.
3. Trim leading and trailing whitespace.
4. Render that line as the signature.

This is intentionally imperfect for multiline declarations, but it is
predictable, syntax-only, cheap, and easy to test. It also avoids designing a
signature-specific mini-language before the rest of the extraction model is
stable.

Future versions can improve signature extraction without changing the basic
outline entry model. See [Future: Rich Signatures](#future-rich-signatures).

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

### Source Organization Boundary

The extractor should preserve source organization instead of normalizing code into a
semantic graph. Relationship-bearing syntax stays in signatures and ordinary entries:

- `class A extends Base implements C {}` is a class entry whose signature keeps the
  relationship syntax.
- `impl Foo for A {}` is its own item with direct member entries; its methods are not
  regrouped onto `A`.

Members are direct structural children of top-level items: fields, methods,
constructors, enum variants, trait/interface/type members, module/namespace
declarations, and the JS/TS function-body helper declarations described in the command
doc. The initial model should not recursively expose arbitrary nested blocks, closures,
local variables, or expressions.

## Extraction Strategy

Extraction must be data-driven. The command should not have Rust match arms such
as "if language is Rust, match `function_item`". Built-in support is a bundled
extractor catalog. User and custom-language support is additional extractor YAML
loaded by `--outline-rules`.

An extractor starts with an ast-grep rule-core object. The `rule`,
`constraints`, `utils`, and `transform` fields should be the same rule-core
fields ast-grep already uses. Outline should not invent a second query language.

### Rule Design Direction

The rule catalog should not encode exported and non-exported forms as separate
item extractors. For example, these two Rust declarations should not require two
mostly duplicated item rules:

```rust
struct Foo {}
pub struct Foo {}
```

Instead:

1. One item matcher selects the declaration node.
2. The extractor derives `name`, `signature`, and `isExported` from the matched
   node and configured metadata.
3. The final entry remains one top-level item.

`isImport` is a boolean marker for import/dependency items. `isExported` is a
boolean predicate for top-level items. `isPublic` is a member-only boolean
predicate.

Predicate rule objects are evaluated with the extracted item node as the
candidate. Normal ast-grep relational rules such as `has` and `inside` keep
their usual meaning relative to that candidate.

Boolean derivation fields follow one rule:

```text
field omitted by extractor       output field is absent
field present and rule matches   output field is true
field present and rule misses    output field is false
field set to true                output field is true
field set to false               output field is false
```

This lets a language opt into member publicness without inventing
public/private/internal categories. For example, Rust can declare `isPublic` for
members and mark `pub`, `pub(crate)`, or `pub(super)` members as true. A language
with no useful member publicness rule can omit `isPublic` entirely.

Rust top-level example:

```yaml
extractors:
  - id: rust-struct
    language: Rust
    role: item
    symbolType: struct
    node:
      rule:
        kind: struct_item
    name:
      field: name
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

Rust import example:

```yaml
extractors:
  - id: rust-use
    language: Rust
    role: item
    symbolType: module
    isImport: true
    node:
      rule:
        kind: use_declaration
    name:
      text: normalized
    target:
      text: normalized
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

Rust member publicness example:

```yaml
extractors:
  - id: rust-field
    language: Rust
    role: member
    symbolType: field
    node:
      rule:
        kind: field_declaration
    name:
      field: name
    isPublic:
      has:
        regex: '^pub\b'
```

JavaScript and TypeScript export wrappers should prefer deriving `export` from
context over matching a separate exported declaration selector:

```yaml
extractors:
  - id: ts-class
    language: TypeScript
    role: item
    symbolType: class
    node:
      rule:
        kind: class_declaration
    name:
      field: name
    isExported:
      inside:
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

This YAML is illustrative, not a committed schema. The important design decision
is the separation:

```text
ast-grep rule      selects candidate syntax.
metadata extract   derives fields from the selected syntax.
item flags          set isImport/isExported on top-level items.
member logic       attaches direct member entries.
renderer           chooses names/signatures/digest/expanded text.
```

Extractor metadata should describe outline-specific fields:

```text
id            Stable extractor id for diagnostics.
language      Any `SgLang`: built-in language or registered custom language.
symbolType    Output symbol type.
node          ast-grep rule object that selects the candidate node.
name          How to resolve the display name.
signature     How to derive a source-like declaration signature.
role          Outline placement: `item` or `member`.
isImport      Whether a top-level item is an import/dependency edge.
isExported    Boolean or predicate for public/module surface membership.
isPublic      Optional member-only boolean predicate.
target        Optional module/package/path target for import/export edges.
alias         Optional renamed-from symbol for import/export edges.
```

The exact YAML schema is intentionally still open. The rule format needs enough
structure to express the fields above, but it should stay much smaller than a
general-purpose query language.

## What To Avoid

### Avoid Export-Specific Duplicated Item Rules

Bad direction:

```yaml
- id: rust-struct
  role: item
  node:
    rule: { kind: struct_item }

- id: rust-exported-struct
  role: item
  isExported: true
  node:
    rule:
      all:
        - kind: struct_item
        - regex: '^pub\s+struct'
```

This duplicates the core definition rule and gets worse for every language,
construct, and export variation.

### Avoid A Large New Mini-Language

The outline rule format should not become a second query language with many
custom expression types. ast-grep already has rule syntax. Outline-specific
metadata should stay small and purpose-built.

Prefer a small set of extraction helpers that are hard to express as ast-grep
rules:

- get tree-sitter field text, such as `field: name`.
- get the first source line from the matched node as a signature.
- inspect language-specific modifiers around the matched node.
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
3. Compile and run each extractor's `node.rule` ast-grep rule against the parsed AST.
4. For each match, produce a candidate entry:
   - source range from matched node.
   - AST kind from matched node.
   - symbol type from extractor metadata.
   - outline placement from `role`.
   - name from a field, metavariable, or edge normalizer.
   - signature from the first non-empty line of the matched node.
   - target/alias from import/export normalizer when applicable.
5. Derive field-local values from syntax:
   - import syntax can set `isImport`.
   - top-level export syntax or language public-surface syntax can set `isExported`.
   - member syntax can set `isPublic`.
6. Deduplicate entries by range, symbol type, name, and edge target.
7. Merge duplicate top-level items by range, symbol type, name, target, and
   alias, preserving `isImport` and `isExported`.
8. Attach direct members by syntax containment and derive member publicness where
   the language exposes it.
9. Sort entries in source order.
10. Pass the file model to CLI filtering and rendering.

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
2. Write one or more outline extractor entries with
   `language: <custom-language-name>`.
3. Run outline with `--outline-rules <FILE>`.

Conceptual example:

```yaml
extractors:
  - id: mylang-function
    language: mylang
    symbolType: function
    node:
      rule:
        pattern: def $NAME($$$ARGS) $$$BODY
    name:
      metavariable: NAME
    role: item
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
   name, generic parameters, parameters, return type, and receiver. A
   TemplateFix-like formatter could then assemble those components into a
   display signature. This should be limited to producing signature/detail text,
   not arbitrary structured metadata fields.

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

## Open Questions

- What is the smallest concrete rule schema that supports field extraction,
  field-local derivation objects, and members without becoming a DSL?
- Should import/export edge normalization be configured per language or
  implemented as a small set of built-in normalizers referenced by rules?
- Which import/export edge normalizers should ship as built-ins for common alias
  syntaxes such as TypeScript `export { foo as bar }` and Python `import x as y`?
- Should custom-language rules be allowed to declare member publicness and
  import/export flags, or should custom languages initially support only items,
  names, and signatures?
