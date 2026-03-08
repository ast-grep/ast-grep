# Parameterized Utils Design

This note describes the current implementation in `crates/config`. It is not a proposal.

## Goal

Parameterized utils let a utility rule accept one or more rule arguments:

```yaml
utils:
  wrap(BODY):
    matches: BODY

rule:
  matches:
    wrap:
      BODY:
        kind: number
```

The implementation keeps parameterized utils close to ordinary utils:

- the matcher AST is still `Rule`
- `matches` is still `Rule::Matches(ReferentRule)`
- there is no separate template AST
- the only extra runtime state is a scoped binding frame for argument rules

## Core Representation

Two storage forms exist in registration:

- local zero-arg utils: `Rule`
- local parameterized utils: `Def<Rule>`
- global zero-arg rules: `RuleCore`
- global parameterized rules: `Def<RuleCore>`

`Def<M>` is just:

- `params: Vec<String>`
- `matcher: M`

A `matches` reference is always represented by `ReferentRule`:

- `rule_id: String`
- `args: Arc<HashMap<String, Arc<Rule>>>`

So plain and parameterized calls share the same runtime type.

## Name Resolution

For bare `matches: NAME`, the implementation resolves in this order:

1. current parameter binding
2. local util/rule
3. global rule

This is intentionally lexical. A parameter name shadows same-named local/global utils.

For `matches: { NAME: { ... } }`:

- `NAME` must be a declared parameterized util/global rule
- calling a parameter as if it were a util is rejected
- all declared arguments must be provided
- unknown argument names are rejected

## Deserialization

`DeserializeEnv` carries two pieces of scope information:

- `local_utils: HashMap<String, Vec<String>>`
- `current_params: Option<Arc<HashSet<String>>>`

That is enough to:

- validate arity at call sites
- reject `PARAM(...)`
- keep dependency walking and cycle checks aware of parameter names

There is no eager expansion of parameterized utils into a new rule tree.

## Runtime Matching

Runtime matching uses binding frames, not tree expansion.

When a parameterized util is called:

1. `ReferentRule` pushes `name -> Rule` bindings into a thread-local frame
2. the stored template body is matched directly
3. a bare `matches: NAME` first checks the binding frame
4. if `NAME` is bound, the bound rule is matched under the parent frame

This gives lexical behavior for nested parameterized calls without cloning the whole rule tree.

## Local vs Global Env Behavior

Local utils and the YAML rule share the same `MetaVarEnv`.

That means:

- local util metavariables can affect YAML rule matching
- same-name metavariables between the YAML rule and local utils are in the same scope

Global rules are different. They match in an isolated local `MetaVarEnv`.

That means:

- internal global metavariables do not affect YAML rule matching
- internal global metavariables are not exported back to the caller
- for parameterized global rules, only vars coming from caller-supplied argument rules are exported

## `defined_vars`

`defined_vars` is intentionally coarse for local utils because they are file-scoped.

Current behavior:

- YAML rule `defined_vars`
- plus all local zero-arg util vars
- plus all local parameterized util body vars
- plus constraint vars

For a parameterized call itself, `ReferentRule::defined_vars()` is only the union of vars defined by its argument rules.

For global parameterized rules, this matches runtime export behavior: only argument-rule vars come back to the caller.

## `potential_kinds`

`MissingPotentialKinds` remains a hard requirement for `RuleConfig`.

The implementation therefore keeps `potential_kinds` conservative.

Rule:

- if kind inference reaches `matches: PARAM-RULE`, it stops and returns `None`

This is deliberate. The code comment in `ReferentRule::potential_kinds()` documents the decision.

Implications:

- caller-supplied rule arguments do not participate in kind inference
- users must provide stable kinds elsewhere if they want pruning to stay precise
- typical fixes are:
  - add a `kind` guard in the util body
  - add a `kind` guard around the util call site

Because parameter refs collapse to `None`, composite caches can stay static:

- relational rules contribute `None`
- `All` and `Any` keep their normal cached `potential_kinds`
- `RuleCore` caches `kinds` again

To make those caches correct, deserialization installs the current parameter-name scope while building a rule. This prevents a same-named local/global util from being used accidentally during cache construction.

## Cycles

Cycle handling is still syntactic.

- utility dependency ordering is computed during deserialization
- `check_cyclic` also walks argument rules
- parameter names are excluded from dependency edges
- call-site cycles created through argument rules are rejected when lowering the call
- a utility cannot call itself through its argument rules, either directly or transitively

There is no runtime recursion detector for parameterized utils.

## Why This Shape

This design keeps the feature small:

- one matcher AST
- one referent matcher type
- no template expansion engine
- no template-only runtime matcher

The extra complexity is limited to:

- deserialization-time scope tracking for params
- runtime binding frames for argument rules
- conservative `potential_kinds` behavior for `matches: PARAM-RULE`
