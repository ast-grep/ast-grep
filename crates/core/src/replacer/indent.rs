#![warn(clippy::doc_overindented_list_items)]
/**
  This module is for indentation-sensitive replacement.

  Ideally, structural search and replacement should all be based on AST.
  But this means our changed AST need to be pretty-printed by structural rules,
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
  Following the same path, we can define a `extract_with_deindent` for steps 1,2
*/
use crate::source::Content;
use std::borrow::Cow;
use std::cmp::Ordering;
use std::ops::Range;

/// We assume NEW_LINE, TAB, SPACE is only one code unit.
/// This is sufficiently true for utf8, utf16 and char.
fn get_new_line<C: Content>() -> C::Underlying {
  C::decode_str("\n")[0].clone()
}
fn get_space<C: Content>() -> C::Underlying {
  C::decode_str(" ")[0].clone()
}

const MAX_LOOK_AHEAD: usize = 512;

/// Represents how we de-indent matched meta var.
pub enum DeindentedExtract<'a, C: Content> {
  /// If meta-var is only one line, no need to de-indent/re-indent
  SingleLine(&'a [C::Underlying]),
  /// meta-var's has multiple lines, may need re-indent
  MultiLine(&'a [C::Underlying], usize),
}

/// Returns DeindentedExtract for later de-indent/re-indent.
pub fn extract_with_deindent<C: Content>(content: &C, range: Range<usize>) -> DeindentedExtract<C> {
  let extract_slice = content.get_range(range.clone());
  // no need to compute indentation for single line
  if !extract_slice.contains(&get_new_line::<C>()) {
    return DeindentedExtract::SingleLine(extract_slice);
  }
  let indent = get_indent_at_offset::<C>(content.get_range(0..range.start));
  DeindentedExtract::MultiLine(extract_slice, indent)
}

fn deindent_slice<'a, C: Content>(
  slice: &'a [C::Underlying],
  content: &'a C,
  start: usize,
) -> DeindentedExtract<'a, C> {
  if !slice.contains(&get_new_line::<C>()) {
    return DeindentedExtract::SingleLine(slice);
  }
  let indent = get_indent_at_offset::<C>(content.get_range(0..start));
  DeindentedExtract::MultiLine(slice, indent)
}

pub fn formatted_slice<'a, C: Content>(
  slice: &'a [C::Underlying],
  content: &'a C,
  start: usize,
) -> Cow<'a, [C::Underlying]> {
  let deindent = deindent_slice(slice, content, start);
  indent_lines(0, deindent)
}

pub fn indent_lines<C: Content>(
  indent: usize,
  extract: DeindentedExtract<C>,
) -> Cow<[C::Underlying]> {
  use DeindentedExtract::*;
  let (lines, original_indent) = match extract {
    SingleLine(line) => return Cow::Borrowed(line),
    MultiLine(lines, ind) => (lines, ind),
  };
  match original_indent.cmp(&indent) {
    // if old and new indent match, just return old lines
    Ordering::Equal => Cow::Borrowed(lines),
    // need strip old indent
    Ordering::Greater => Cow::Owned(remove_indent::<C>(original_indent - indent, lines)),
    // need add missing indent
    Ordering::Less => Cow::Owned(indent_lines_impl::<C, _>(
      indent - original_indent,
      lines.split(|b| *b == get_new_line::<C>()),
    )),
  }
}

fn indent_lines_impl<'a, C, Lines>(indent: usize, mut lines: Lines) -> Vec<C::Underlying>
where
  C: Content + 'a,
  Lines: Iterator<Item = &'a [C::Underlying]>,
{
  let mut ret = vec![];
  let space = get_space::<C>();
  let leading: Vec<_> = std::iter::repeat(space).take(indent).collect();
  // first line never got indent
  if let Some(line) = lines.next() {
    ret.extend(line.iter().cloned());
  };
  let new_line = get_new_line::<C>();
  for line in lines {
    ret.push(new_line.clone());
    ret.extend(leading.clone());
    ret.extend(line.iter().cloned());
  }
  ret
}

/// returns 0 if no indent is found before the offset
/// either truly no indent exists, or the offset is in a long line
pub fn get_indent_at_offset<C: Content>(src: &[C::Underlying]) -> usize {
  let lookahead = src.len().max(MAX_LOOK_AHEAD) - MAX_LOOK_AHEAD;

  let mut indent = 0;
  let new_line = get_new_line::<C>();
  let space = get_space::<C>();
  // TODO: support TAB. only whitespace is supported now
  for c in src[lookahead..].iter().rev() {
    if *c == new_line {
      return indent;
    }
    if *c == space {
      indent += 1;
    } else {
      indent = 0;
    }
  }
  // lookahead == 0 means we have indentation at first line.
  if lookahead == 0 && indent != 0 {
    indent
  } else {
    0
  }
}

// NOTE: we assume input is well indented.
// following line's should have fewer indentation than initial line
fn remove_indent<C: Content>(indent: usize, src: &[C::Underlying]) -> Vec<C::Underlying> {
  let indentation: Vec<_> = std::iter::repeat(get_space::<C>()).take(indent).collect();
  let new_line = get_new_line::<C>();
  let lines: Vec<_> = src
    .split(|b| *b == new_line)
    .map(|line| match line.strip_prefix(&*indentation) {
      Some(stripped) => stripped,
      None => line,
    })
    .collect();
  lines.join(&new_line).to_vec()
}

#[cfg(test)]
mod test {
  use super::*;

  fn test_deindent(source: &str, expected: &str, offset: usize) {
    let source = source.to_string();
    let expected = expected.trim();
    let start = source[offset..]
      .chars()
      .take_while(|n| n.is_whitespace())
      .count()
      + offset;
    let trailing_white = source
      .chars()
      .rev()
      .take_while(|n| n.is_whitespace())
      .count();
    let end = source.chars().count() - trailing_white;
    let extracted = extract_with_deindent(&source, start..end);
    let result_bytes = indent_lines::<String>(0, extracted);
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
    test_deindent(src, expected, 0);
  }

  #[test]
  fn test_first_line_indent_deindent() {
    // note this indentation has no newline
    let src = r"  def test():
    pass";
    let expected = r"
def test():
  pass";
    test_deindent(src, expected, 0);
  }

  #[test]
  fn test_space_in_middle_deindent() {
    let src = r"
a = lambda:
  pass";
    let expected = r"
lambda:
  pass";
    test_deindent(src, expected, 4);
  }

  #[test]
  fn test_middle_deindent() {
    let src = r"
  a = lambda:
    pass";
    let expected = r"
lambda:
  pass";
    test_deindent(src, expected, 6);
  }

  #[test]
  fn test_nested_deindent() {
    let src = r"
def outer():
  def test():
    pass";
    let expected = r"
def test():
  pass";
    test_deindent(src, expected, 13);
  }

  #[test]
  fn test_no_deindent() {
    let src = r"
def test():
  pass
";
    test_deindent(src, src, 0);
  }

  #[test]
  fn test_malformed_deindent() {
    let src = r"
  def test():
pass
";
    let expected = r"
def test():
pass
";
    test_deindent(src, expected, 0);
  }

  #[test]
  fn test_long_line_no_deindent() {
    let src = format!("{}abc\n  def", " ".repeat(MAX_LOOK_AHEAD + 1));
    test_deindent(&src, &src, 0);
  }

  fn test_replace_with_indent(target: &str, start: usize, inserted: &str) -> String {
    let target = target.to_string();
    let replace_lines = DeindentedExtract::MultiLine(inserted.as_bytes(), 0);
    let indent = get_indent_at_offset::<String>(&target.as_bytes()[..start]);
    let ret = indent_lines::<String>(indent, replace_lines);
    String::from_utf8(ret.to_vec()).unwrap()
  }

  #[test]
  fn test_simple_replace() {
    let target = "";
    let inserted = "def abc(): pass";
    let actual = test_replace_with_indent(target, 0, inserted);
    assert_eq!(actual, inserted);
    let inserted = "def abc():\n  pass";
    let actual = test_replace_with_indent(target, 0, inserted);
    assert_eq!(actual, inserted);
  }

  #[test]
  fn test_indent_replace() {
    let target = "  ";
    let inserted = "def abc(): pass";
    let actual = test_replace_with_indent(target, 2, inserted);
    assert_eq!(actual, "def abc(): pass");
    let inserted = "def abc():\n  pass";
    let actual = test_replace_with_indent(target, 2, inserted);
    assert_eq!(actual, "def abc():\n    pass");
    let target = "    "; // 4 spaces, but insert at 2
    let actual = test_replace_with_indent(target, 2, inserted);
    assert_eq!(actual, "def abc():\n    pass");
    let target = "    "; // 4 spaces, insert at 4
    let actual = test_replace_with_indent(target, 4, inserted);
    assert_eq!(actual, "def abc():\n      pass");
  }

  #[test]
  fn test_leading_text_replace() {
    let target = "a = ";
    let inserted = "def abc(): pass";
    let actual = test_replace_with_indent(target, 4, inserted);
    assert_eq!(actual, "def abc(): pass");
    let inserted = "def abc():\n  pass";
    let actual = test_replace_with_indent(target, 4, inserted);
    assert_eq!(actual, "def abc():\n  pass");
  }

  #[test]
  fn test_leading_text_indent_replace() {
    let target = "  a = ";
    let inserted = "def abc(): pass";
    let actual = test_replace_with_indent(target, 6, inserted);
    assert_eq!(actual, "def abc(): pass");
    let inserted = "def abc():\n  pass";
    let actual = test_replace_with_indent(target, 6, inserted);
    assert_eq!(actual, "def abc():\n    pass");
  }
}
