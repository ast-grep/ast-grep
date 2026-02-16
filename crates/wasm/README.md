# @ast-grep/wasm

WebAssembly build of [ast-grep](https://ast-grep.github.io/) for use in browsers and Node.js.

This package provides the same API as [`@ast-grep/napi`](https://www.npmjs.com/package/@ast-grep/napi) but runs in any JavaScript environment that supports WebAssembly, including browsers, Deno, and edge runtimes.

## Installation

```bash
yarn add @ast-grep/wasm web-tree-sitter
```

`web-tree-sitter` is a required peer dependency.

## Usage

```js
import { initializeTreeSitter, setupParser, parse, kind } from '@ast-grep/wasm'

// 1. Initialize the tree-sitter WASM runtime (once)
await initializeTreeSitter()

// 2. Load a language parser WASM binary
await setupParser('javascript', '/path/to/tree-sitter-javascript.wasm')

// 3. Parse and search code
const sg = parse('javascript', 'console.log("hello world")')
const node = sg.root().find('console.log($ARG)')
console.log(node.getMatch('ARG').text()) // "hello world"
```

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

## Supported Languages

bash, c, cpp, csharp, css, elixir, go, haskell, html, java, javascript, json, kotlin, lua, nix, php, python, ruby, rust, scala, swift, tsx, typescript, yaml

## API Reference

See the full [ast-grep API documentation](https://ast-grep.github.io/reference/api.html).

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
