# ast-grep-language

[![Crates.io](https://img.shields.io/crates/v/ast-grep-language.svg)](https://crates.io/crates/ast-grep-language)
[![Website](https://img.shields.io/badge/ast--grep-Website-red?logoColor=red)](https://ast-grep.github.io/)

This crate provides language support for [ast-grep](https://ast-grep.github.io/), including tree-sitter parsers for 25+ programming languages.

## Installation

Add this to your `Cargo.toml`:

```toml
[dependencies]
ast-grep-language = "0.40"
```

## Features

By default, all language parsers are included. You can selectively enable only the languages you need to reduce binary size.

### Default (all languages)

```toml
[dependencies]
ast-grep-language = "0.40"
```

### Selective Languages

To include only specific languages, disable default features and enable the ones you need:

```toml
[dependencies]
ast-grep-language = { version = "0.40", default-features = false, features = ["lang-typescript", "lang-rust"] }
```

### Available Language Features

| Feature | Language | Tree-sitter Parser |
|---------|----------|-------------------|
| `lang-bash` | Bash | tree-sitter-bash |
| `lang-c` | C | tree-sitter-c |
| `lang-cpp` | C++ | tree-sitter-cpp |
| `lang-csharp` | C# | tree-sitter-c-sharp |
| `lang-css` | CSS | tree-sitter-css |
| `lang-elixir` | Elixir | tree-sitter-elixir |
| `lang-go` | Go | tree-sitter-go |
| `lang-haskell` | Haskell | tree-sitter-haskell |
| `lang-hcl` | HCL | tree-sitter-hcl |
| `lang-html` | HTML | tree-sitter-html |
| `lang-java` | Java | tree-sitter-java |
| `lang-javascript` | JavaScript | tree-sitter-javascript |
| `lang-json` | JSON | tree-sitter-json |
| `lang-kotlin` | Kotlin | tree-sitter-kotlin |
| `lang-lua` | Lua | tree-sitter-lua |
| `lang-nix` | Nix | tree-sitter-nix |
| `lang-php` | PHP | tree-sitter-php |
| `lang-python` | Python | tree-sitter-python |
| `lang-ruby` | Ruby | tree-sitter-ruby |
| `lang-rust` | Rust | tree-sitter-rust |
| `lang-scala` | Scala | tree-sitter-scala |
| `lang-solidity` | Solidity | tree-sitter-solidity |
| `lang-swift` | Swift | tree-sitter-swift |
| `lang-typescript` | TypeScript/TSX | tree-sitter-typescript |
| `lang-yaml` | YAML | tree-sitter-yaml |

### Meta Features

| Feature | Description |
|---------|-------------|
| `builtin-parser` | Enables all language parsers (default) |
| `napi-lang` | Enables CSS, HTML, JavaScript, and TypeScript (for Node.js bindings) |

## Usage

```rust
use ast_grep_language::{SupportLang, LanguageExt};

// Parse a language from string
let lang: SupportLang = "rust".parse().unwrap();

// Use the language to parse source code
let source = "fn main() {}";
let root = lang.ast_grep(source);

// Find patterns in code
let found = root.root().find("fn $NAME() {}");
```

## Reducing Binary Size

If you're building a tool that only needs to support specific languages, you can significantly reduce your binary size by enabling only the required language features:

```toml
# Example: TypeScript-only tool
[dependencies]
ast-grep-language = { version = "0.40", default-features = false, features = ["lang-typescript"] }

# Example: Web languages only
[dependencies]
ast-grep-language = { version = "0.40", default-features = false, features = [
  "lang-javascript",
  "lang-typescript",
  "lang-html",
  "lang-css"
]}
```

## Resources

- [ast-grep Official Website](https://ast-grep.github.io/)
- [API Usage Guide](https://ast-grep.github.io/guide/api-usage.html)
- [GitHub Repository](https://github.com/ast-grep/ast-grep)

## License

This project is licensed under the MIT license.
