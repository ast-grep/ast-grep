# Quick Start

You can unleash `ast-grep`'s power at your finger tips within few keystrokes in command line!

Let's try its power of  by rewriting some code in a moderately large codebase: [TypeScript](https://github.com/microsoft/TypeScript/).

Our task is to rewrite old defensive code that checks nullable nested method calls to the new shiny [optional chaining operator](https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Operators/Optional_chaining) `?.`.

## Installation
First, install `ast-grep`. It is distributed by npm and cargo. You can also build it [from source](https://github.com/ast-grep/ast-grep#installation).

```shell
# install via npm
npm i @ast-grep/cli -g

# install via cargo
cargo install ast-grep
```

The binary command, `sg`, should be available now. Let's try it with `--help`.


```shell
sg --help
```

Optionally, you can grab TypeScript source code if you want to follow the tutorial. Or you can apply the magic to your own code.

```shell
git clone git@github.com:microsoft/TypeScript.git --depth 1
```

## Pattern
Then search the occurrence of looking up a method from a nested structure. `ast-grep` uses **pattern** to find similar code.
Think it as the pattern in our old-friend `grep` but it matches AST node instead of text.
We can write pattern as if write ordinary code. It will match all code that has the same syntactical structure.

For example, the following pattern code

```javascript
obj.val && obj.val()
```

will match all the following code, regardless of white spaces or new lines.

```javascript
obj.val && obj.val() // verbatim match, of course
obj.val    &&     obj.val() // this matches, too

// this matches as well!
const result = obj.val &&
   obj.val()
```

Matching based exactly on AST is cool, but we certainly want to use flexible pattern to match code with infinite possibility.
We can use **meta variable** to match any single AST node. Meta variable begins with `$` sign with upper case letters following, e.g. `$METAVAR`.
Think it as REGEX dot `.`, except it is not textual.

We can write this pattern to find all property checking code.

```javascript
$PROP && $PROP()
```

It is a valid `ast-grep` pattern! We can use it in command line! Use `pattern` argument to specify our target
and also `lang` is needed to tell ast-grep our target code language.

```shell
sg --pattern '$PROP && $PROP()' --lang ts TypeScript/src # path to TypeScript source
```

## Rewrite

Cool? Now we can use this pattern to refactor TypeScript source!

```shell
# pattern and language argument support short form
sg -p '$PROP && $PROP()' \
   --rewrite '$PROP?.()' \
   --interactive \
   -l ts \
   TypeScript/src
```

ast-grep will start an interactive session to let you choose if you want to apply the patch.
Press `y` to accept the change!


That's it! You have refactored TypeScript's repository in minutes. Congratulation!

Hope you enjoy the power of AST editing in plain programming language pattern. Our next step is to know more about the pattern code.
