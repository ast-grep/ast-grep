# @ast-grep/wasm

WebAssembly build of [ast-grep](https://ast-grep.github.io/) for use in browsers and Node.js.

This package provides the same API as [`@ast-grep/napi`](https://www.npmjs.com/package/@ast-grep/napi) but runs in any JavaScript environment that supports WebAssembly, including browsers, Deno, and edge runtimes.

## Installation

```bash
yarn add @ast-grep/wasm web-tree-sitter
```

`web-tree-sitter` is a required peer dependency.

### Vite

When using Vite, you need to make `tree-sitter.wasm` available in your public directory. You can automate this with a `postinstall` script in your `package.json` (see [web-tree-sitter setup](https://github.com/tree-sitter/tree-sitter/tree/master/lib/binding_web#setup)):

```json
"postinstall": "cp node_modules/web-tree-sitter/tree-sitter.wasm public"
```

## Usage

Unlike `@ast-grep/napi`, this package has no predefined language support. All languages must be registered at runtime by loading their tree-sitter WASM parser.

```js
import { initializeTreeSitter, registerDynamicLanguage, parse, kind } from '@ast-grep/wasm'

// 1. Initialize the tree-sitter WASM runtime (once)
await initializeTreeSitter()

// 2. Register a language by loading its WASM parser
await registerDynamicLanguage({
  javascript: { libraryPath: '/path/to/tree-sitter-javascript.wasm' },
})

// 3. Parse and search code
const sg = parse('javascript', 'console.log("hello world")')
const node = sg.root().find('console.log($ARG)')
console.log(node.getMatch('ARG').text()) // "hello world"
```

### Registering Languages

`registerDynamicLanguage` accepts a map of language name to registration config. It can be called multiple times to add or update languages.

```js
await registerDynamicLanguage({
  javascript: { libraryPath: '/path/to/tree-sitter-javascript.wasm' },
  python: {
    libraryPath: '/path/to/tree-sitter-python.wasm',
    expandoChar: 'µ',  // custom expando char for languages where $ is special
  },
})
```

The `expandoChar` option sets the character used internally to represent metavariables (defaults to `$`). Use a different character for languages where `$` is a valid identifier character (e.g. PHP, Bash).

### Pattern Matching

```js
// By pattern string
sg.root().find('console.log($$$ARGS)')

// By kind number
const k = kind('javascript', 'call_expression')
sg.root().find(k)

// By rule config (same as YAML rules)
sg.root().find({
  rule: { pattern: 'console.log($A)' },
  constraints: { A: { kind: 'string' } },
})
```

### Code Rewriting

```js
const match = sg.root().find('console.log($A)')
const edit = match.replace('console.error($A)')
const newCode = sg.root().commitEdits([edit])
```

## API Reference

### Top-level functions

#### `initializeTreeSitter(): Promise<void>`

Initializes the tree-sitter WASM runtime. Must be called once before any other function.

#### `registerDynamicLanguage(langs: Record<string, { libraryPath: string, expandoChar?: string }>): Promise<void>`

Registers one or more language parsers by loading their WASM binaries. Can be called multiple times; existing languages are updated.

#### `parse(lang: string, src: string): SgRoot`

Parses source code and returns an `SgRoot` instance. Throws if the language has not been registered.

#### `kind(lang: string, kindName: string): number`

Returns the numeric kind ID for a named node type in the given language. Useful for matching by node kind.

#### `pattern(lang: string, patternStr: string): object`

Compiles a pattern string into a rule config object (equivalent to `{ rule: { pattern: patternStr } }`). Useful for building rule configs programmatically.

#### `dumpPattern(lang: string, patternStr: string, selector?: string, strictness?: string): PatternTree`

Dumps the internal structure of a pattern for inspection and debugging. Returns a tree showing how ast-grep parses the pattern, including source positions and node kinds.

- `selector`: optional kind name for contextual patterns (e.g. `'field_definition'`)
- `strictness`: one of `"cst"`, `"smart"` (default), `"ast"`, `"relaxed"`, `"signature"`, `"template"`

Each `PatternTree` node has:
- `kind`: the tree-sitter node kind string
- `pattern`: `"metaVar"`, `"terminal"`, or `"internal"`
- `isNamed`: whether the node is a named node
- `text`: source text (for metavar and terminal nodes)
- `children`: child `PatternTree` nodes
- `start`, `end`: `{ line, column }` positions in the pattern source

### `SgRoot`

Represents the parsed tree of code.

#### `root(): SgNode`

Returns the root `SgNode`.

#### `filename(): string`

Returns `"anonymous"` when the instance is created via `parse`.

#### `getInnerTree(): Tree`

Returns the underlying `web-tree-sitter` `Tree` object. Useful for low-level inspection or debugging.

### `SgNode`

Represents a single AST node.

#### Position and info

| Method | Description |
|--------|-------------|
| `range()` | Returns `{ start, end }` where each is `{ line, column, index }` |
| `isLeaf()` | True if the node has no children |
| `isNamed()` | True if the node is a named (non-anonymous) node |
| `isNamedLeaf()` | True if the node is a named node with no named children |
| `kind()` | Returns the node kind string |
| `is(kind: string)` | True if the node kind equals `kind` |
| `text()` | Returns the source text of the node |
| `id()` | Returns the unique node ID |

#### Searching

| Method | Description |
|--------|-------------|
| `find(matcher)` | Returns the first descendant matching the matcher, or `undefined` |
| `findAll(matcher)` | Returns all descendants matching the matcher |

Matchers can be a pattern string, a kind number (from `kind()`), or a rule config object.

#### Relational matchers

| Method | Description |
|--------|-------------|
| `matches(matcher)` | True if the node itself matches |
| `inside(matcher)` | True if the node is inside an ancestor matching the matcher |
| `has(matcher)` | True if the node has a descendant matching the matcher |
| `precedes(matcher)` | True if the node comes before a sibling matching the matcher |
| `follows(matcher)` | True if the node comes after a sibling matching the matcher |

#### Match environment

| Method | Description |
|--------|-------------|
| `getMatch(name: string)` | Returns the node bound to a metavariable (e.g. `$VAR`) |
| `getMultipleMatches(name: string)` | Returns nodes bound to a multi-metavariable (e.g. `$$$ARGS`) |
| `getTransformed(name: string)` | Returns the string value of a transformed variable |

#### Tree traversal

| Method | Description |
|--------|-------------|
| `children_nodes()` | Returns all child nodes |
| `parent_node()` | Returns the parent node, or `undefined` |
| `child(nth: number)` | Returns the nth child, or `undefined` |
| `ancestors()` | Returns all ancestors from parent to root |
| `next_node()` | Returns the next sibling, or `undefined` |
| `nextAll()` | Returns all following siblings |
| `prev()` | Returns the previous sibling, or `undefined` |
| `prevAll()` | Returns all preceding siblings |
| `field(name: string)` | Returns the child node for a named field, or `undefined` |
| `fieldChildren(name: string)` | Returns all child nodes for a named field |

#### Editing

| Method | Description |
|--------|-------------|
| `replace(text: string)` | Creates a `WasmEdit` replacing this node's range with `text` |
| `commitEdits(edits: WasmEdit[])` | Applies edits to the node's text and returns the new source string |

`WasmEdit` has `start_pos`, `end_pos` (character offsets), and `inserted_text`. These fields can be modified before calling `commitEdits`.

## Building from Source

Requires [wasm-pack](https://rustwasm.github.io/wasm-pack/installer/).

```bash
# For browser (ES module)
yarn build

# For Node.js (CommonJS)
yarn build:nodejs

# For bundlers
yarn build:bundler
```

## Testing

```bash
yarn install
```

### Rust WASM tests

```bash
yarn test
```

> **Note:** `wasm-pack test --node` runs the test harness from a temporary directory.
> The `test` script sets `NODE_PATH=$PWD/node_modules` so that `web-tree-sitter` and
> parser WASM files can be resolved. If you run `wasm-pack test --node` directly,
> set `NODE_PATH` yourself:
>
> ```bash
> NODE_PATH=$PWD/node_modules wasm-pack test --node
> ```

### JavaScript tests

```bash
yarn test:js
```

This builds the WASM package for Node.js and runs AVA tests against it.
