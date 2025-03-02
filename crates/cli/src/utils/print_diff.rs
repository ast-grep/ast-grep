use crate::print::ColorArg;

use ansi_term::{Color, Style};
use anyhow::Result;
use similar::{ChangeTag, DiffOp, TextDiff};

use std::fmt::Display;
use std::io::Write;

// TODO: use termcolor instead
/// Print diff styles for colored output
#[derive(Default, Clone)]
pub struct DiffStyles {
  pub line_num: Style,
  // diff insert style
  pub insert: Style,
  pub insert_emphasis: Style,
  // diff deletion style
  pub delete: Style,
  pub delete_emphasis: Style,
}

impl DiffStyles {
  pub fn colored() -> Self {
    static THISTLE1: Color = Color::Fixed(225);
    static SEA_GREEN: Color = Color::Fixed(158);
    static RED: Color = Color::Fixed(161);
    static GREEN: Color = Color::Fixed(35);
    let insert = Style::new().fg(GREEN);
    let delete = Style::new().fg(RED);
    Self {
      line_num: Style::new().dimmed(),
      insert,
      insert_emphasis: insert.on(SEA_GREEN).bold(),
      delete,
      delete_emphasis: delete.on(THISTLE1).bold(),
    }
  }
  fn no_color() -> Self {
    Self::default()
  }

  pub fn print_diff(
    &self,
    old: &str,
    new: &str,
    writer: &mut impl Write,
    context: usize,
  ) -> Result<()> {
    print_diff(self, old, new, writer, context)
  }
}

impl From<ColorArg> for DiffStyles {
  fn from(color: ColorArg) -> Self {
    if color.should_use_color() {
      Self::colored()
    } else {
      Self::no_color()
    }
  }
}

fn index_display(index: Option<usize>, style: Style, width: usize) -> impl Display {
  let index_str = match index {
    None => format!("{:width$}", ""),
    Some(idx) => format!("{:<width$}", idx + 1), // 0-based index -> 1-based line num
  };
  style.paint(index_str)
}

// TODO: currently diff print context is three lines before/after the match.
// This is suboptimal. We should use function/class as the enclosing scope to print relevant lines. See #155
fn compute_header(group: &[DiffOp]) -> String {
  let old_start = group[0].old_range().start;
  let new_start = group[0].new_range().start;
  let (old_len, new_len) = group.iter().fold((0, 0), |(o, n), op| {
    (o + op.old_range().len(), n + op.new_range().len())
  });
  format!(
    "@@ -{},{} +{},{} @@",
    old_start, old_len, new_start, new_len
  )
}

fn print_diff(
  styles: &DiffStyles,
  old: &str,
  new: &str,
  writer: &mut impl Write,
  context: usize,
) -> Result<()> {
  let diff = TextDiff::from_lines(old, new);
  for group in diff.grouped_ops(context) {
    let op = group.last().unwrap();
    let old_width = op.old_range().end.checked_ilog10().unwrap_or(0) as usize + 1;
    let new_width = op.new_range().end.checked_ilog10().unwrap_or(0) as usize + 1;
    let header = compute_header(&group);
    writeln!(writer, "{}", Color::Blue.paint(header))?;
    for op in group {
      for change in diff.iter_inline_changes(&op) {
        let (sign, s, em, line_num) = match change.tag() {
          ChangeTag::Delete => ("-", styles.delete, styles.delete_emphasis, styles.delete),
          ChangeTag::Insert => ("+", styles.insert, styles.insert_emphasis, styles.insert),
          ChangeTag::Equal => (" ", Style::new(), Style::new(), styles.line_num),
        };
        write!(
          writer,
          "{} {}│{}",
          index_display(change.old_index(), line_num, old_width),
          index_display(change.new_index(), line_num, new_width),
          s.paint(sign),
        )?;
        for (emphasized, value) in change.iter_strings_lossy() {
          if emphasized {
            write!(writer, "{}", em.paint(value))?;
          } else {
            write!(writer, "{}", s.paint(value))?;
          }
        }
        if change.missing_newline() {
          writeln!(writer)?;
        }
      }
    }
  }
  Ok(())
}
