/*!
This module is for indentation-sensitive replacement.

Ideally, structral search and replacement should all be based on AST.
But this means our changed AST need to be pretty-printed by structral rules,
which we don't have enough resource to support. An indentation solution is used.

The algorithm is quite complicated, uncomprehensive, sluggish and buggy.
But let's walk through it by example.

consider this code
```
if (true) {
  a(
    1
      + 2
      + 3
  )
}
```

and this pattern and replacement

```
// pattern
a($B)
// replacement
c(
  $B
)
```

We need to compute the relative indentation of the captured meta-var.
When we insert the meta-var into replacement, keep the relative indent intact,
while also respecting the replacement indent.
Finally, the whole replacement should replace the matched node
in a manner that maintains the indentation of the source.

We need to consider multiple indentations.
Key concepts here:
* meta-var node: in this case `$B` in pattern/replacement, or `1+2+3` in source.
* matched node: in this case `a($B)` in pattern, a(1 + 2 + 3)` in source
* meta-var source indentation: `$B` matches `1+2+3`, the first line's indentation in source code is 4.
* meta-var replacement indentation: in this case 2
* matched node source indentation: in this case 2

## Extract Meta-var with de-indent
1. Initial meta-var node B text:
The meta-var source indentation for `$B` is 4.
However, meta-var node does not have the first line indentation.
```
1
      + 2
      + 3
```
2. Deindent meta-var node B, except first line:
De-indenting all lines following the first line by 4 spaces gives us this relative code layout.

```
1
  + 2
  + 3
```

## Insert meta-var into replacement with re-indent

3. Re-indent by meta-var replacement indentation.
meta-var node $B occurs in replace with first line indentation of 2.
We need to re-indent the meta-var code before replacement, except the first line
```
1
    + 2
    + 3
```

4. Insert meta-var code in to replacement
```
c(
  1
    + 2
    + 3
)
```

## Insert replacement into source with re-indent

5. Re-indent the replaced template code except first line
The whole matched node first line indentation is 2.
We need to reindent the replacement code by 2, except the first line.
```
c(
    1
      + 2
      + 3
  )
```

6. Inserted replacement code to original tree

```
if (true) {
  c(
    1
      + 2
      + 3
  )
}
```
*/
