<p align=center>
  <img src="https://ast-grep.github.io/logo.svg" alt="ast-grep"/>
</p>

<p align="center">
   <img src="https://github.com/ast-grep/ast-grep/actions/workflows/coverage.yaml/badge.svg" alt="coverage badge"/>
   <a href="https://app.codecov.io/gh/ast-grep/ast-grep"><img src="https://codecov.io/gh/ast-grep/ast-grep/branch/main/graph/badge.svg?token=37VX8H2EWV"/></a>
   <a href="https://discord.gg/4YZjf6htSQ" target="_blank"><img alt="Discord" src="https://img.shields.io/discord/1107749847722889217?label=Discord"></a>
   <img src="https://img.shields.io/github/stars/ast-grep/ast-grep?style=social" alt="Badge"/>
   <img src="https://img.shields.io/github/forks/ast-grep/ast-grep?style=social" alt="Badge"/>
   <img alt="GitHub Sponsors" src="https://img.shields.io/github/sponsors/HerringtonDarkholme?style=social">
</p>


## ast-grep(sg)

ast-grep(sg) is a CLI tool for code structural search, lint, and rewriting.

## Introduction
ast-grep is a AST-based tool to search code by pattern code. Think it as your old-friend `grep` but it matches AST nodes instead of text.
You can write patterns as if you are writing ordinary code. It will match all code that has the same syntactical structure.
You can use `$` sign + upper case letters as wildcard, e.g. `$MATCH`, to match any single AST node. Think it as REGEX dot `.`, except it is not textual.

Try the [online playground](https://ast-grep.github.io/playground.html) for a taste!

## Demo

![output](https://user-images.githubusercontent.com/2883231/183275066-8d9c342f-46cb-4fa5-aa4e-b98aac011869.gif)

## Installation
You can install it from [npm](https://docs.npmjs.com/downloading-and-installing-node-js-and-npm), [pip](https://pypi.org/), [cargo](https://doc.rust-lang.org/cargo/getting-started/installation.html), [homebrew](https://brew.sh/) or [scoop](https://scoop.sh/)!

```bash
npm install --global @ast-grep/cli
pip install ast-grep-cli
cargo install ast-grep

# install via homebrew, thank @henryhchchc
brew install ast-grep

# install via scoop, thank @brian6932
scoop install main/ast-grep
```
Or you can build ast-grep from source. You need install rustup, clone the repository and then
```bash
cargo install --path ./crates/cli
```
[Packages](https://repology.org/project/ast-grep/versions) are available on other platforms too.

## Command line usage example

ast-grep has following form.
```
sg --pattern 'var code = $PATTERN' --rewrite 'let code = new $PATTERN' --lang ts
```

### Example

* [Rewrite code in null coalescing operator](https://twitter.com/Hchan_mgn/status/1547061516993699841?s=20&t=ldDoj4U2nq-FRKQkU5GWXA)

```bash
sg -p '$A && $A()' -l ts -r '$A?.()'
```

* [Rewrite](https://twitter.com/Hchan_mgn/status/1561802312846278657) [Zodios](https://github.com/ecyrbe/zodios#migrate-to-v8)
```bash
sg -p 'new Zodios($URL,  $CONF as const,)' -l ts -r 'new Zodios($URL, $CONF)' -i
```

* [Implement eslint rule using YAML.](https://twitter.com/Hchan_mgn/status/1560108625460355073)


## Sponsor
![Sponsors](https://raw.githubusercontent.com/HerringtonDarkholme/sponsors/main/sponsorkit/sponsors.svg)

If you find ast-grep interesting and useful for your work, please [buy me a coffee](https://github.com/sponsors/HerringtonDarkholme)
so I can spend more time on the project!

## Feature Highlight

ast-grep's core is an algorithm to search and replace code based on abstract syntax tree produced by tree-sitter.
It can help you to do lightweight static analysis and massive scale code manipulation in an intuitive way.

Key highlights:

* An intuitive pattern to find and replace AST.
ast-grep's pattern looks like ordinary code you would write every day. (You can call the pattern is isomorphic to code).

* jQuery like API for AST traversal and manipulation.

* YAML configuration to write new linting rules or code modification.

* Written in compiled language, with tree-sitter based parsing and utilizing multiple cores.

* Beautiful command line interface :)

ast-grep's vision is to democratize abstract syntax tree magic and to liberate one from cumbersome AST programming!

* If you are an open source library author, ast-grep can help your library users adopt breaking changes more easily.
* if you are a tech lead in your team, ast-grep can help you enforce code best practice tailored to your business need.
* If you are a security researcher, ast-grep can help you write rules much faster.


## CLI Screenshot

### Search
| Feature | Command | Screenshot |
| ------- | ------- | ---------- |
| Search  | `sg -p 'Some($A)' -l rs` | ![image](https://github.com/ast-grep/ast-grep/assets/2883231/002db3a2-8a79-4838-ad5c-563634183c3f) |
| Rewrite | `sg -p '$F && $F($$$ARGS)' -r '$F?.($$$ARGS)' -l ts` | ![image](https://github.com/ast-grep/ast-grep/assets/2883231/ad9394d8-3aea-4b96-8d54-6e01f06174d2)|
| Report  | `sg scan` | ![image](https://user-images.githubusercontent.com/2883231/187094977-fd544d4b-64de-4bba-8bea-8c0de047b352.png) |
