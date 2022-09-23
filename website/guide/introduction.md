# What is ast-grep?

## Motivation

ast-grep is a new AST based tool for managing your code, at massive scale.

Developing with AST is tedious and frustrating. Consider this "hello-world" level task: matching `console.log` in JavaScript using Babel. We will need to write code like below.

```javascript
path.parentPath.isMemberExpression() &&
path.parentPath.get('object').isIdentifier({ name: 'console' }) &&
path.parentPath.get('property').isIdentifier({ name: 'log' })
```

This snippet deserves a detailed explanation for beginners. Even for experienced developers, authoring this snippet also requires a lot of looking up references.

ast-grep solves the problem by providing a simple core mechanism: using code to search code with the same pattern.
Consider it as same as `grep` but based on AST instead of text.

In comparison to Babel, we can complete this hello-world task in ast-grep

```javascript
console.log
```

See [playground](https://ast-grep.github.io/ast-grep/playground.html) in action!

Upon the simple pattern code, we can build a series of operators to compose complex matching rules for various scenarios.

Though we use JavaScript in our introduction, ast-grep is not language specific. It is a _polyglot_ tool backed by the renowned library [tree-sitter](https://tree-sitter.github.io/).
The idea of ast-grep can be applied to many other languages!

## Use case

We can use ast-grep as searcher, linter and rewriter.

* **Searcher**: As a command line tool in your terminal, ast-grep, `sg`, can precisely search code based on AST, running through ten thousand files in sub seconds.
* **Linter**: You can also use ast-grep as a linter. Thanks to the flexible rule configuration, adding a new customized rule is more intuitive and straightforward. It also has a pretty error reporting out of box
* **Rewrite Library**: ast-grep provide jQuery like utility methods to traverse and manipulate syntax tree. Besides, you can also use operators to compose complex matching from simple patterns.


## Features

There are a lot of existing tools that looks like ast-grep, notable predecessor including [Semgrep](https://semgrep.dev/), comby, shisho, gogocode.

What makes astgrep stands out is:

### Performance

It is written in Rust, a native language and utilize multiple cores. (It can even beat ag when searching simple pattern). Astgrep can handle tens of thousands files in seconds.

### Progressiveness
You can start from writing a oneliner to rewrite code at command line with minimal investment. Later if you see some code smell recurrently appear in your projects, you can write a linter rule in YAML with a few patterns combined. Finally if you are a library author or framework designer, astgrep provide programmatic interface to rewrite or transpile code efficiently.

### Pragmatism
ast-grep comes with batteries included. Interactive code modification is available. Linter and language server work out of box when you install the command line tool. Astgrep is also shipped with test à for rule authors.

