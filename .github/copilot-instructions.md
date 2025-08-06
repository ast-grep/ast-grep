# ast-grep Development Instructions

ast-grep is a CLI tool for structural search, lint, and code rewriting using abstract syntax trees. It's written in Rust with TypeScript/Node.js (NAPI) and Python bindings.

**Always reference these instructions first and fallback to search or bash commands only when you encounter unexpected information that does not match the info here.**

## Working Effectively

### Essential Setup and Build Commands
Run these commands in the repository root (the top-level directory of your local clone, e.g. `<repo-root>`) in order:

- Check Rust toolchain: `rustc --version && cargo --version`
- Quick validation: `cargo check` - takes 2m 40s, validates dependencies and compilation
- Debug build: `cargo build` - takes 42 seconds. **NEVER CANCEL.** Set timeout to 120+ seconds.
- Release build: `cargo build --release` - takes 2m 26s. **NEVER CANCEL.** Set timeout to 180+ seconds.
- Test suite: `cargo test` - takes 48 seconds, runs 94+ tests. **NEVER CANCEL.** Set timeout to 120+ seconds.

### Linting and Formatting
- Format check: `cargo fmt --all -- --check` - takes <1 second
- Lint check: `cargo clippy --all-targets --all-features --workspace --release --locked -- -D clippy::all` - takes 30 seconds. **NEVER CANCEL.** Set timeout to 60+ seconds.

### Running the CLI
- Main binary: `./target/release/ast-grep --help`
- Short alias: `./target/debug/sg --version` (requires ast-grep in PATH: `PATH="$(pwd)/target/debug:$PATH"`)
- Test pattern matching: `./target/release/ast-grep -p 'console.log($MSG)' -l js /path/to/file.js`
- Test rewriting: `./target/release/ast-grep -p 'var $VAR = $VALUE' -r 'let $VAR = $VALUE' -l js /path/to/file.js`

## Node.js/NAPI Bindings

### Setup and Build
- Navigate to: `cd crates/napi`
- Install dependencies: `yarn install` - takes 41 seconds. **NEVER CANCEL.** Set timeout to 90+ seconds.
- Debug build: `yarn build:debug` - takes 37 seconds. **NEVER CANCEL.** Set timeout to 90+ seconds.
- Test suite: `yarn test` - takes 5 seconds, runs 42 tests. **NEVER CANCEL.** Set timeout to 30+ seconds.
- Lint (requires network): `yarn lint` - may fail due to network restrictions, this is expected

**Note:** The NAPI bindings require Node.js >= 10. Tested with Node.js v20.19.4 and yarn 1.22.22.

## Python Bindings

### Setup and Build
- Install maturin: `pip install maturin`
- Build wheel: `maturin build -m crates/pyo3/Cargo.toml` - takes 21 seconds. **NEVER CANCEL.** Set timeout to 60+ seconds.
- Install wheel: `pip install target/wheels/ast_grep_py-*.whl`
- Test import: `python3 -c "import ast_grep_py; print('Python bindings work!')"`

**Note:** Python tests require module to be built in development mode, which needs virtualenv. Use wheel installation instead.

## Validation Scenarios

### After Making Changes - ALWAYS run these scenarios:
1. **Basic CLI functionality test:**
   ```bash
   echo 'console.log("hello world")' > /tmp/test.js
   ./target/release/ast-grep -p 'console.log($MSG)' -l js /tmp/test.js
   # Should output: /tmp/test.js with line match
   ```

2. **Rewrite functionality test:**
   ```bash
   echo 'var x = 5;' > /tmp/test.js
   ./target/release/ast-grep -p 'var $VAR = $VALUE' -r 'let $VAR = $VALUE' -l js /tmp/test.js
   # Should show diff replacing var with let
   ```

3. **Configuration-based scanning test:**
   ```bash
   mkdir -p /tmp/test-project/src && cd /tmp/test-project
   echo 'var y = 10;' > src/test.js
   cat > sgconfig.yml << 'EOF'
   rule:
     id: no-var
     message: Use let or const instead of var
     severity: error
     language: javascript
     rule:
       pattern: var $VAR = $VALUE
     fix: let $VAR = $VALUE
   EOF
   ./target/release/ast-grep scan src/
   ```

4. **LSP server startup test:**
   ```bash
   timeout 5 ./target/release/ast-grep lsp || echo "LSP started and stopped as expected"
   # Should show configuration error (expected) or start LSP server
   ```

### Pre-commit validation checklist:
- [ ] `cargo fmt --all -- --check` (format check)
- [ ] `cargo clippy --all-targets --all-features --workspace --release --locked -- -D clippy::all` (lint check)
- [ ] `cargo test` (test suite)
- [ ] Run validation scenarios above
- [ ] If touching NAPI code: `cd crates/napi && yarn test`

## Common Issues and Solutions

### Build Issues
- **"No such file or directory" for sg binary:** Ensure ast-grep is in PATH or use full path to sg binary
- **Network errors during yarn lint:** Expected in restricted environments, dprint needs internet access
- **Python test failures:** Expected, tests require development build which needs virtualenv

### Performance Expectations
- **CRITICAL:** Builds can take 2+ minutes - **NEVER CANCEL** build or test commands
- Set timeouts: Debug build (120s+), Release build (180s+), Tests (120s+), Clippy (60s+)
- Large dependency downloads on first build are normal

## Repository Structure

### Key Directories
- `crates/cli/` - Main CLI application
- `crates/core/` - Core AST processing logic
- `crates/config/` - Configuration handling
- `crates/language/` - Language-specific parsers
- `crates/napi/` - Node.js bindings
- `crates/pyo3/` - Python bindings
- `crates/lsp/` - Language Server Protocol implementation
- `fixtures/` - Test fixtures
- `npm/` - NPM package distribution

### Important Files
- `Cargo.toml` - Workspace configuration
- `crates/cli/Cargo.toml` - Main CLI package config
- `crates/napi/package.json` - Node.js package config
- `pyproject.toml` - Python package config
- `.github/workflows/` - CI/CD pipelines

## Supported Languages
ast-grep supports 20+ programming languages including JavaScript, TypeScript, Python, Rust, Go, Java, C/C++, and more through tree-sitter parsers.

## Additional Notes
- The project uses tree-sitter for parsing
- Multiple output formats supported: colored terminal, JSON, etc.
- Interactive mode available for applying fixes
- LSP server for editor integration
- WASM version available for web usage