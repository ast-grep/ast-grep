use ansi_term::{Color, Style};
use anyhow::{Error, Result};

use std::fmt;
use std::path::PathBuf;

const DOC_SITE_HOST: &str = "https://ast-grep.github.io";
const CONFIG_GUIDE: Option<&str> = Some("/guide/rule-config.html");
const CLI_USAGE: Option<&str> = Some("/reference/cli.html");
const TEST_GUIDE: Option<&str> = Some("/guide/test-rule.html");
const EDITOR_INTEGRATION: Option<&str> = Some("/guide/editor-integration.html");

/// AppError stands for ast-grep command line usage.
/// It provides abstraction around exit code, context,
/// message, potential fix and reference link.
#[derive(Debug, Clone)]
pub enum ErrorContext {
  // Config
  ReadConfiguration,
  ParseConfiguration,
  WalkRuleDir(PathBuf),
  ReadRule(PathBuf),
  ParseRule(PathBuf),
  ParseTest(PathBuf),
  StartLanguageServer,
  // Edit
  OpenEditor,
}

impl ErrorContext {
  fn exit_code(&self) -> i32 {
    use ErrorContext::*;
    match self {
      ReadConfiguration | ReadRule(_) | WalkRuleDir(_) => 2,
      ParseTest(_) | ParseRule(_) | ParseConfiguration => 5,
      OpenEditor => 126,
      _ => 1,
    }
  }
}

impl fmt::Display for ErrorContext {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    let err_msg = ErrorMessage::from_context(self);
    write!(f, "{}", err_msg.title)
  }
}

// guideline: https://twitter.com/mgechev/status/1405019854906834944
// * Use consistent terminology
// * Be clear and concise
// * Provide context
// * Suggest a fix
struct ErrorMessage {
  title: String,
  description: String,
  link: Option<&'static str>,
}

impl ErrorMessage {
  fn new<T: ToString, D: ToString>(title: T, description: D, link: Option<&'static str>) -> Self {
    Self {
      title: title.to_string(),
      description: description.to_string(),
      link,
    }
  }

  fn from_context(ctx: &ErrorContext) -> ErrorMessage {
    use ErrorContext::*;
    match ctx {
      ReadConfiguration => Self::new(
        "Cannot read configuration.",
        "Please add an sgconfig.yml configuration file in the project root to run the scan command.",
        CONFIG_GUIDE,
      ),
      ParseConfiguration => Self::new(
        "Cannot parse configuration",
        "The sgconfig.yml is not a valid configuration file. Please refer to doc and fix the error.",
        CONFIG_GUIDE,
      ),
      WalkRuleDir(dir) => Self::new(
        format!("Cannot read rule directory {}", dir.display()),
        "The rule directory cannot be read or traversed",
        None,
      ),
      ReadRule(file) => Self::new(
        format!("Cannot read rule {}", file.display()),
        "The rule file either does not exist or cannot be opened.",
        CONFIG_GUIDE,
      ),
      ParseRule(file) => Self::new(
        format!("Cannot parse rule {}", file.display()),
        "The file is not a valid ast-grep rule. Please refer to doc and fix the error.",
        CONFIG_GUIDE,
      ),
      ParseTest(file) => Self::new(
        format!("Cannot parse test case {}", file.display()),
        "The file is not a valid ast-grep test case. Please refer to doc and fix the error.",
        TEST_GUIDE,
      ),
      StartLanguageServer => Self::new(
        "Cannot start language server.",
        "Please see language server logging file.",
        EDITOR_INTEGRATION,
      ),
      OpenEditor => Self::new(
        "Cannot open file in editor.",
        "Please check if the editor is installed and the EDITOR environment variable is correctly set.",
        CLI_USAGE,
      ),
    }
  }
}

pub fn exit_with_error(error: Error) -> Result<()> {
  if let Some(e) = error.downcast_ref::<clap::Error>() {
    e.exit()
  }
  if let Some(e) = error.downcast_ref::<ErrorContext>() {
    let error_fmt = ErrorFormat {
      context: e,
      inner: &error,
    };
    eprintln!("{error_fmt}");
    std::process::exit(e.exit_code())
  }
  // use anyhow's default error reporting
  Err(error)
}

struct ErrorFormat<'a> {
  context: &'a ErrorContext,
  inner: &'a Error,
}

impl<'a> fmt::Display for ErrorFormat<'a> {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    let ErrorMessage {
      title,
      description,
      link,
    } = ErrorMessage::from_context(self.context);
    let bold = Style::new().bold();
    let error = Color::Red.paint("Error:");
    let message = bold.paint(title);
    writeln!(f, "{error} {message}")?;
    let help = Color::Blue.paint("Help:");
    writeln!(f, "{help} {description}")?;
    if let Some(url) = link {
      let reference = Style::new().bold().dimmed().paint("See also:");
      let link = format!(
        "\u{1b}]8;;{DOC_SITE_HOST}{url}\u{1b}\\{}{}\u{1b}]8;;\u{1b}\\",
        Color::Cyan.italic().paint(DOC_SITE_HOST),
        Color::Cyan.italic().paint(url)
      );
      writeln!(f, "{reference} {link}")?;
    }
    writeln!(f)?;
    writeln!(f, "{} Caused by", Color::Red.paint("×"))?;
    // skip root error
    for err in self.inner.chain().skip(1) {
      let prefix = Color::Red.paint("╰▻");
      writeln!(f, "{prefix} {err}")?;
    }
    Ok(())
  }
}
