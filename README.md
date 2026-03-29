# ast-grep-dart

A standalone fork of [ast-grep](https://github.com/ast-grep/ast-grep) with first-class **Dart language support**.

> **Note:** This is an independently maintained fork. It is **not** affiliated with or endorsed by the upstream ast-grep project.

## What is ast-grep?

ast-grep is a CLI tool for code structural search, lint, and rewriting based on [abstract syntax trees](https://dev.to/balapriya/abstract-syntax-tree-ast-explained-in-plain-english-1h38). Think of it as `grep`, but matching AST nodes instead of text. You write patterns as ordinary code, and use `$`-prefixed uppercase names (e.g. `$MATCH`) as wildcards.

This fork adds Dart as a fully supported language alongside the 24+ languages already supported by upstream.

## Installation

### npm (recommended)

```bash
npm install --global @bramburn/ast-grep-dart
```

### From source

```bash
cargo install --path ./crates/cli --locked
```

### pip

```bash
pip install ast-grep-dart-cli
```

## Usage

```bash
# Search for a pattern in Dart files
ast-grep --pattern 'var $NAME = $VALUE' --lang dart

# Rewrite code
ast-grep -p 'var $A = $B' -l dart -r 'final $A = $B'
```

Try the upstream [online playground](https://ast-grep.github.io/playground.html) for non-Dart languages.

## Feature Highlights

- **AST-based pattern matching** — patterns look like ordinary code
- **Dart support** — first-class tree-sitter Dart parsing
- **jQuery-like API** for AST traversal and manipulation (NAPI)
- **YAML configuration** for lint rules and code modifications
- **Fast** — written in Rust, multi-core, tree-sitter powered
- **25+ languages** including Dart, TypeScript, Python, Go, Rust, and more

## Packages

| Package | Description |
|---------|-------------|
| `@bramburn/ast-grep-dart` | CLI binary (npm) |
| `@bramburn/ast-grep-dart-napi` | Node.js NAPI bindings |
| `ast-grep-dart-cli` | CLI binary (PyPI) |

## Upstream

This fork is based on [ast-grep](https://github.com/ast-grep/ast-grep) by Herrington Darkholme, licensed under the MIT License. See [LICENSE](./LICENSE) for details.

## License

MIT — see [LICENSE](./LICENSE).

## Maintainer

Bhavesh Ramburn ([b@icelabz.co.uk](mailto:b@icelabz.co.uk))
