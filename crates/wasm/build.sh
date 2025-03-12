#!/bin/bash

export OUT_DIR=pkg
export CFLAGS_wasm32_unknown_unknown="-I$(pwd)/wasm-sysroot -Wbad-function-cast -Wcast-function-type -fno-builtin" RUSTFLAGS="-Zwasm-c-abi=spec"
export AST_GREP_RULE='{"id":"fix-wasm-js-node","language":"javascript","rule":{"pattern":"module_or_path = fetch(module_or_path);","inside":{"pattern":"function __wbg_init($$$) {$$$}","stopBy":"end"}},"fix":"if (!!process.versions.node) {\n  const fs = await import(\"fs/promises\");\n  module_or_path = fs.readFile(module_or_path);\n} else {\n  module_or_path = fetch(module_or_path);\n}\n"}'

wasm-pack build --release --target web --out-dir $OUT_DIR --features "exhaustive-parser" \
	-Z build-std=panic_abort,std -Z build-std-features=panic_immediate_abort

# Make the WASM module work with Node.js as Node does not support fetch for file:// URLs
cargo run --manifest-path ../cli/Cargo.toml -- scan --inline-rules "$AST_GREP_RULE" -U $OUT_DIR/ast_grep_wasm.js
