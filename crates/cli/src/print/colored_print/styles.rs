// copied from termcolor
use super::ColorChoice;
use ansi_term::{Color, Style};
use anyhow::Result;
use std::env;

// warn[rule-id]: rule message here.
// |------------|------------------|
//    header            message
#[derive(Default)]
pub struct RuleStyle {
  // header style
  pub error: Style,
  pub warning: Style,
  pub info: Style,
  pub hint: Style,
  // message style
  pub message: Style,
  pub note: Style,
}

// TODO: use termcolor instead
#[derive(Default)]
pub struct PrintStyles {
  // print match color
  pub file_path: Style,
  pub matched: Style,
  pub line_num: Style,
  // diff insert style
  pub insert: Style,
  pub insert_emphasis: Style,
  // diff deletion style
  pub delete: Style,
  pub delete_emphasis: Style,
  pub rule: RuleStyle,
}

impl PrintStyles {
  fn colored() -> Self {
    static THISTLE1: Color = Color::Fixed(225);
    static SEA_GREEN: Color = Color::Fixed(158);
    static RED: Color = Color::Fixed(161);
    static GREEN: Color = Color::Fixed(35);
    let insert = Style::new().fg(GREEN);
    let delete = Style::new().fg(RED);
    Self {
      file_path: Color::Cyan.italic(),
      matched: Color::Red.bold(),
      line_num: Style::new().dimmed(),
      insert,
      insert_emphasis: insert.on(SEA_GREEN).bold(),
      delete,
      delete_emphasis: delete.on(THISTLE1).bold(),
      rule: RuleStyle {
        error: Color::Red.bold(),
        warning: Color::Yellow.bold(),
        info: Style::new().bold(),
        hint: Style::new().dimmed().bold(),
        note: Style::new().italic(),
        message: Style::new().bold(),
      },
    }
  }
  fn no_color() -> Self {
    Self::default()
  }

  pub fn push_matched_to_ret(&self, ret: &mut String, matched: &str) -> Result<()> {
    use std::fmt::Write;
    // TODO: use intersperse
    let mut lines = matched.lines();
    if let Some(line) = lines.next() {
      write!(ret, "{}", self.matched.paint(line))?;
    } else {
      return Ok(());
    }
    for line in lines {
      ret.push('\n');
      write!(ret, "{}", self.matched.paint(line))?;
    }
    Ok(())
  }
}
impl From<ColorChoice> for PrintStyles {
  fn from(color: ColorChoice) -> Self {
    if should_use_color(&color) {
      Self::colored()
    } else {
      Self::no_color()
    }
  }
}

/// Returns true if we should attempt to write colored output.
pub fn should_use_color(color: &ColorChoice) -> bool {
  match *color {
    // TODO: we should check if ansi is supported on windows console
    ColorChoice::Always => true,
    ColorChoice::AlwaysAnsi => true,
    ColorChoice::Never => false,
    // NOTE tty check is added
    ColorChoice::Auto => atty::is(atty::Stream::Stdout) && env_allows_color(),
  }
}

fn env_allows_color() -> bool {
  match env::var_os("TERM") {
    // On Windows, if TERM isn't set, then we should not automatically
    // assume that colors aren't allowed. This is unlike Unix environments
    None => {
      if !cfg!(windows) {
        return false;
      }
    }
    Some(k) => {
      if k == "dumb" {
        return false;
      }
    }
  }
  // If TERM != dumb, then the only way we don't allow colors at this
  // point is if NO_COLOR is set.
  if env::var_os("NO_COLOR").is_some() {
    return false;
  }
  true
}
