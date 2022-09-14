# TODO:

## Core
- [x] Add replace
- [x] Add find_all
- [x] Add metavar char customization
- [x] Add per-language customization
- [x] Add support for vec/sequence matcher
- [x] View node in context
- [x] implement iterative DFS mode
- [ ] Investigate perf heuristic (e.g. match fixed-string)
- [ ] Group matching rules based on root pattern kind id
- [ ] Remove unwrap usage and implement error handling

## Metavariable Matcher
- [x] Regex
- [x] Pattern
- [x] Kind
- [ ] Use CoW to optimize MetaVarEnv

## Operators/Combinators
- [x] every / all
- [x] either / any
- [x] inside
- [x] has
- [x] follows
- [x] precedes

## CLI
- [x] match against files in directory recursively
- [x] interactive mode
- [x] as dry run mode (listing all rewrite)
- [x] inplace edit mode
- [ ] no-color mode
- [ ] JSON output
- [ ] execute remote rules

## Config
- [x] support YAML config rule
- [x] Add support for severity
- [x] Add support for error message
- [x] Add support for error labels
- [x] Add support for fix

## Binding
- [ ] NAPI binding
- [x] WASM binding
- [ ] Python binding

## Playground
- [x] build a playground based on WASM binding
- [x] build YAML config for WASM playground
- [ ] URL sharing
- [ ] add fix/rewrite

## LSP
- [x] Add LSP command
- [ ] implement LSP incremental
- [ ] add code action

## Builtin Ruleset
- [ ] Migrate some ESLint rule (or RSLint rule)
