use crate::print::ColorArg;
use ansi_term::{Color, Style};

// TODO: use termcolor instead
/// Print diff styles for colored output
#[derive(Default)]
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
