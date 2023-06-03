/*!
This module is for indentation-sensitive replacement.

Ideally, structral search and replacement should all be based on AST.
But this means our changed AST need to be pretty-printed by structral rules,
which we don't have enough resource to support. An indentation solution is used.

The algorithm is quite complicated, uncomprehensive, sluggish and buggy.
But let's walk through it by example.

consider this code
```ignore
if (true) {
  a(
    1
      + 2
      + 3
  )
}
```

and this pattern and replacement

```ignore
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
```ignore
1
      + 2
      + 3
```
2. Deindent meta-var node B, except first line:
De-indenting all lines following the first line by 4 spaces gives us this relative code layout.

```ignore
1
  + 2
  + 3
```

## Insert meta-var into replacement with re-indent

3. Re-indent by meta-var replacement indentation.
meta-var node $B occurs in replace with first line indentation of 2.
We need to re-indent the meta-var code before replacement, except the first line
```ignore
1
    + 2
    + 3
```

4. Insert meta-var code in to replacement
```ignore
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
```ignore
c(
    1
      + 2
      + 3
  )
```

6. Inserted replacement code to original tree

```ignore
if (true) {
  c(
    1
      + 2
      + 3
  )
}
```

The steps 3,4 and steps 5,6 are similar. We can define a `replace_with_indent` to it.
Following the same path, we can define a `extract_with_deindent` for steps 1,2.
*/

use crate::source::Content;
use std::ops::Range;

pub trait IndentationSensitiveContent: Content {
  fn replace_with_indent(
    &self,
    start: usize,
    replace_lines: Vec<Vec<Self::Underlying>>,
  ) -> Vec<Self::Underlying>;
  /// Returns None if we don't need to use complicated deindent.
  fn extract_with_deindent(&self, range: Range<usize>) -> Option<Vec<Vec<Self::Underlying>>>;
}

const MAX_LOOK_AHEAD: usize = 512;

impl IndentationSensitiveContent for String {
  fn replace_with_indent(
    &self,
    start: usize,
    replace_lines: Vec<Vec<Self::Underlying>>,
  ) -> Vec<Self::Underlying> {
    let indent = get_indent_at_offset(self, start).unwrap_or(0);
    indent_lines(indent, replace_lines)
  }
  // TODO: should use single_line, no_leading_indent, de_indented
  // None is not enough
  fn extract_with_deindent(&self, range: Range<usize>) -> Option<Vec<Vec<Self::Underlying>>> {
    // no need to compute indentation for single line
    if !self[range.clone()].contains('\n') {
      return None;
    }
    let indent = get_indent_at_offset(self, range.start)?;
    Some(remove_indent(indent, &self[range]))
  }
}

fn indent_lines(indent: usize, lines: Vec<Vec<u8>>) -> Vec<u8> {
  let mut ret = vec![];
  let mut lines = lines.into_iter();
  let leading = " ".repeat(indent);
  if let Some(line) = lines.next() {
    ret.extend(leading.bytes());
    ret.extend(line);
  };
  for line in lines {
    ret.push(b'\n');
    ret.extend(leading.bytes());
    ret.extend(line);
  }
  ret
}

/// returns None if no newline char is found before the offset
/// this happens if the replacement is in a long line
fn get_indent_at_offset(src: &str, start: usize) -> Option<usize> {
  let lookahead = if start > MAX_LOOK_AHEAD {
    start - MAX_LOOK_AHEAD
  } else {
    0
  };

  let mut indent = 0;
  // TODO: support TAB. only whitespace is supported now
  for c in src[lookahead..start].chars().rev() {
    if c == '\n' {
      return if indent == 0 { None } else { Some(indent) };
    }
    if c == ' ' {
      indent += 1;
    } else {
      indent = 0;
    }
  }
  None
}

fn remove_indent(indent: usize, src: &str) -> Vec<Vec<u8>> {
  let mut result = vec![];
  let indentation = " ".repeat(indent);
  for line in src.lines() {
    let s = match line.strip_prefix(&indentation) {
      Some(stripped) => stripped,
      None => line,
    };
    result.push(s.bytes().collect());
  }
  result
}

#[cfg(test)]
mod test {
  use super::*;

  fn test_deindent(source: &str, expected: &str) {
    let source = source.to_string();
    let expected = expected.strip_prefix(|n| n == '\n').unwrap_or(expected);
    let start = source.chars().take_while(|n| n.is_whitespace()).count();
    let end = source.chars().count()
      - source
        .chars()
        .rev()
        .take_while(|n| n.is_whitespace())
        .count();
    let extracted = source.extract_with_deindent(start..end).unwrap();
    let result_bytes = indent_lines(0, extracted);
    let actual = std::str::from_utf8(&result_bytes).unwrap();
    assert_eq!(actual, expected);
  }

  #[test]
  fn test_simple_deindent() {
    let src = r"
  def test():
    pass";
    let expected = r"
def test():
  pass";
    test_deindent(src, expected);
  }
}
