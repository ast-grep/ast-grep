# Pattern Syntax

In this guide we will walk through ast-grep's pattern syntax. The example will be written in JavaScript, but the basic principle will
apply to other languages as well.

## Pattern matching

ast-grep uses pattern code to construct AST tree and match that against target code. The pattern code can search
through the full syntax tree, so pattern can also match nested expression. For example, the pattern `a + 1` can match all the following
code.

```javascript

const b = a + 1

funcCall(a + 1)

deeplyNested({
  target: a + 1
})
```

::: warning
Pattern code must be valid code that tree-sitter can parse.
:::

## Meta Variable
It is usually desirable to write a pattern to match dynamic content.

We can use meta varialbes to match sub expression in pattern.

Meta variables starts with `$` sign, followed its name composed by upper case letters `A-Z`, underscore `_` or digits `1-9`.
`$META_VARIABLE` is a wildcard expression that can match any **single** AST node.

Think it as REGEX dot `.`, except it is not textual.


:::tip Valid meta variables
`$META`, `$META_VARIABLE`, `$META_VAR1`
:::


:::danger Invalid meta variables
`$invalid`, `$Svalue`, `$123`
:::

The pattern `console.log($GREETING)` will match all the following.

```javascript
function tryAstGrep() {
  console.log('Hello World')
}

const multiLineExpression =
  console
   .log('Also matched!')
```

But it will not match these.

```javascript
// console.log(123) in comment is not matched
'console.log(123) in string' // is not matched as well
console.log() // mismatch argument
console.log(a, b) // too many arguments
```

Note, one meta variable `$MATCH` will match one **single** AST node, so the last two `console.log` calls do not match the pattern.
Let's see how we can match multiple AST nodes.

## Multi Meta Variable

We can use `$$$` to match zero or more AST nodes, including function arguments, parameters or statements.


### Function arguments
For example, `console.log($$$)` can match

```javascript
console.log()
console.log('hello world')
console.log('debug: ', key, value)
console.log(...args) // it also matches spread
```

### Function parameters

`function $FUNC($$$) { $$$ }` will match

```javascript
function foo(bar) {
  return bar
}

function noop() {}

function add(a, b, c) {
  return a + b + c
}
```

## Meta Variable Capturing

Meta variable is also similar to [capture group](https://developer.mozilla.org/en-US/docs/Web/JavaScript/Guide/Regular_Expressions/Groups_and_Backreferences) in regular expression.
You can reuse same name meta variables to find previously occurred AST nodes.

For example, the pattern `$A == $A` will have the following result.

```javascript
// will match these patterns
a == a
1 + 1 == 1 + 1
// but will not match these
a == b
1 + 1 == 2
```


## Non capturing match

You can also suppress meta variable capturing. All meta variables with name starting with underscore `_` will not be captured.

```javascript
// Given this pattern

$_FUNC($_FUNC)

// it will match all function call with one argument or spread call
test(a)
testFunc(1 + 1)
testFunc(...args)
```

Note in the example above, even if two meta variables have the same name `$_FUNC`, each occurrence of `$_FUNC` can match different content because the are not captured.

:::info Why use non-capturing match?
This is a useful trick to micro-optimize pattern matching speed, since we don't need to create a [HashMap](https://doc.rust-lang.org/stable/std/collections/struct.HashMap.html) for bookkeeping.
:::
