# AST-Grep

A tool to greatly simplify AST matching and manipulation. Democratize compiler magic and liberate one from cumbersome AST programming! Imagine ast-grep as the jQuery for tree-sitter AST.

## CLI Screenshot

### Search
<img width="796" alt="image" src="https://user-images.githubusercontent.com/2883231/178289737-1b4cdf53-454d-4953-b031-1f9a92996874.png">

### Rewrite
<img width="1296" alt="image" src="https://user-images.githubusercontent.com/2883231/178289574-94a38df7-88fc-4f5e-9293-870091c51902.png">

### Error report
![image](https://user-images.githubusercontent.com/2883231/183095365-15895b64-4b3e-400e-91ed-cf9cdfdd4c32.png)




## TODO:

### Core
- [x] Add replace
- [x] Add find_all
- [x] Add metavar char customization
- [x] Add per-language customization
- [x] Add support for vec/sequence matcher
- [ ] Investigate perf heuristic (e.g. match fixed-string)
- [ ] Group matching rules based on root pattern kind id
- [x] View node in context
- [x] implement iterative DFS mode

## Metavariable Matcher
- [ ] Regex
- [x] Pattern
- [x] Kind

## Rule
- [x] every / all
- [x] either / any
- [x] inside
- [x] has
- [ ] follows
- [ ] precedes

### CLI
- [x] match against files in directory recursively
- [x] interactive mode
- [x] as dry run mode (listing all rewrite)
- [ ] inplace edit mode
- [ ] no-color mode
- [ ] JSON output

### Config
- [x] support YAML config rule
- [x] Add support for severity
- [x] Add support for error message
- [ ] Add support for error labels
- [ ] Add support for fix

### Binding
- [ ] NAPI binding
- [ ] WASM binding
- [ ] Python binding

### Playground
- [ ] build a playground based on WASM binding

### Builtin Ruleset
- [ ] Migrate some ESLint rule (or RSLint rule)
