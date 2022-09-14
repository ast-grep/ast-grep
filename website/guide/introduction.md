# What is ast-grep?

## Motivation

`ast-grep` is a new AST based tool for managing your code, at massive scale.

It can be a searcher, linter and rewriter.

It's core is using code to search code with the same pattern. Consider it as grep but based on ast.

## Features

There are a lot of existing tools that looks like astgrep. Semgrep, comby, shisho, gogocode.

What makes astgrep stands out is

### Performance

It is written in Rust, a native language and utilize multiple cores. (It can even beat ag when searching simple pattern). Astgrep can handle tens of thousands files in seconds.

### Progressiveness
You can start from writing a oneliner to rewrite code at command line with minimal investment. Later if you see some code smell recurrently appear in your projects, you can write a linter rule in YAML with a few patterns combined. Finally if you are a library author or framework designer, astgrep provide programmatic interface to rewrite or transpile code efficiently.

### Pragmatism
ast-grep comes with batteries included. Interactive code modification is available .Linter and language server work out of box when you install the command line tool. Astgrep is also shipped with test à for rule authors.

## Use case

### Searcher
### Linter
### Refactor Tool
### AST library
