# ast-grep playground

## Pre-requisite

It requires tree-sitter.wasm and tree-sitter-{lang}.wasm available in public directory.

Language specific wasm must be built with the same emcc version of the tree-sitter.wasm.

Mismatching emcc version will raise RuntimeError.

## Reference
* https://github.com/tree-sitter/tree-sitter/issues/1593
* https://github.com/tree-sitter/tree-sitter/issues/1829
