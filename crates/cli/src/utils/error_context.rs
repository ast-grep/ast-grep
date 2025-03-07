use ansi_term::{Color, Style};
use anyhow::{Error, Result};

use crossterm::tty::IsTty;
use std::fmt;
use std::path::PathBuf;

const DOC_SITE_HOST: &str = "https://ast-grep.github.io";
const PATTERN_GUIDE: Option<&str> = Some("/guide/pattern-syntax.html");
const CONFIG_GUIDE: Option<&str> = Some("/guide/rule-config.html");
const CONFIG_REFERENCE: Option<&str> = Some("/reference/sgconfig.html");
const PROJECT_GUIDE: Option<&str> = Some("/guide/scan-project.html");
const TOOL_OVERVIEW: Option<&str> = Some("/guide/tooling-overview.html#parse-code-from-stdin");
const CLI_USAGE: Option<&str> = Some("/reference/cli.html");
const TEST_GUIDE: Option<&str> = Some("/guide/test-rule.html");
const UTIL_GUIDE: Option<&str> = Some("/guide/rule-config/utility-rule.html");
const EDITOR_INTEGRATION: Option<&str> = Some("/guide/editor-integration.html");
const LANGUAGE_LIST: Option<&str> = Some("/reference/languages.html");
const PLAYGROUND: Option<&str> = Some("/playground.html");
const CUSTOM_LANG_GUIDE: Option<&str> = Some("/advanced/custom-language.html");
const UTILITY_RULE: Option<&str> = Some("/guide/rule-config/utility-rule.html");

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
  InvalidGlobalUtils,
  GlobPattern,
  BuildGlobs,
  UnrecognizableLanguage(String),
  LangInjection,
  CustomLanguage,
  // Run
  ParsePattern,
  LanguageNotSpecified,
  StdInIsNotInteractive,
  PatternHasError,
  // Scan
  DiagnosticError(usize),
  RuleNotSpecified,
  RuleNotFound(String),
  // LSP
  StartLanguageServer,
  // Edit
  OpenEditor,
  WriteFile(PathBuf),
  // Test
  TestFail(String),
  // New
  ProjectAlreadyExist,
  ProjectNotExist,
  FileAlreadyExist(PathBuf),
  NoTestDirConfigured,
  NoUtilDirConfigured,
  InsufficientCLIArgument(&'static str),
  // Completions
  CannotInferShell,
}

impl ErrorContext {
  fn exit_code(&self) -> i32 {
    use ErrorContext::*;
    // reference: https://mariadb.com/kb/en/operating-system-error-codes/
    match self {
      DiagnosticError(_) => 1,
      ProjectNotExist | LanguageNotSpecified | RuleNotSpecified | RuleNotFound(_) => 2,
      TestFail(_) => 3,
      NoTestDirConfigured | NoUtilDirConfigured => 4,
      ReadConfiguration | ReadRule(_) | WalkRuleDir(_) | WriteFile(_) => 5,
      StdInIsNotInteractive => 6,
      ParseTest(_) | ParseRule(_) | ParseConfiguration | ParsePattern | InvalidGlobalUtils
      | LangInjection => 8,
      GlobPattern | BuildGlobs => 9,
      CannotInferShell => 10,
      ProjectAlreadyExist | FileAlreadyExist(_) => 17,
      InsufficientCLIArgument(_) => 22,
      UnrecognizableLanguage(_) => 33,
      CustomLanguage => 79,
      OpenEditor | StartLanguageServer => 126,
      // soft error
      PatternHasError => 0,
    }
  }

  fn is_soft_error(&self) -> bool {
    self.exit_code() == 0
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
        CONFIG_REFERENCE,
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
      GlobPattern => Self::new(
        "Cannot parse glob pattern in config",
        "The pattern in files/ignore is not a valid glob. Please refer to doc and fix the error.",
        CONFIG_GUIDE,
      ),
      BuildGlobs => Self::new(
        "Cannot build glob from CLI flag",
        "The patterns in --globs is invalid. Please refer to doc and fix the error.",
        CLI_USAGE,
      ),
      LangInjection => Self::new(
        "Cannot parse languageInjections in config",
        "The rule in languageInjections is not valid. Please refer to doc and fix the error.",
        CONFIG_GUIDE,
      ),
      CustomLanguage => Self::new(
        "Cannot load custom language library",
        "The custom language library is not found or cannot be loaded.",
        CUSTOM_LANG_GUIDE,
      ),
      InvalidGlobalUtils => Self::new(
        "Error occurs when parsing global utility rules",
        "Please check the YAML rules inside the rule directory",
        UTILITY_RULE,
      ),
      UnrecognizableLanguage(lang) => Self::new(
        format!("Language `{lang}` is not supported"),
        "Please choose a built-in language or register a custom language in sgconfig.yml.",
        LANGUAGE_LIST,
      ),
      ParseTest(file) => Self::new(
        format!("Cannot parse test case {}", file.display()),
        "The file is not a valid ast-grep test case. Please refer to doc and fix the error.",
        TEST_GUIDE,
      ),
      DiagnosticError(num) => Self::new(
        format!("{num} error(s) found in code."),
        "Scan succeeded and found error level diagnostics in the codebase.",
        None,
      ),
      ParsePattern => Self::new(
        "Cannot parse query as a valid pattern.",
        "The pattern either fails to parse or contains error. Please refer to pattern syntax guide.",
        PATTERN_GUIDE,
      ),
      LanguageNotSpecified => Self::new(
        "Language must be specified for code from StdIn.",
        "Please use `--lang` to specify the code language.",
        TOOL_OVERVIEW,
      ),
      StdInIsNotInteractive => Self::new(
        "Interactive mode is incompatible with parsing code from StdIn.",
        "`--interactive` needs StdIn, but it is used as source code. Please use files as input.",
        TOOL_OVERVIEW,
      ),
      PatternHasError => Self::new(
        "Pattern contains an ERROR node and may cause unexpected results.",
        "ast-grep parsed the pattern but it matched nothing in this run. Try using playground to refine the pattern.",
        PLAYGROUND,
      ),
      RuleNotSpecified => Self::new(
        "Only one rule can scan code from StdIn.",
        "Please use `--rule path/to/rule.yml` to choose the rule.",
        TOOL_OVERVIEW,
      ),
      RuleNotFound(id) => Self::new(
        format!("Rule not found: {}", id),
        format!("Rule with id '{id}' not found in project configuration. Please make sure it exists."),
        TOOL_OVERVIEW,
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
      WriteFile(file) => Self::new(
        format!("Cannot rewrite file {}", file.display()),
        "Fail to apply fix to the file. Skip to next file",
        None,
      ),
      TestFail(message) => Self::new(
        message,
        "You can use ast-grep playground to debug your rules and test cases.",
        PLAYGROUND,
      ),
      ProjectAlreadyExist => Self::new(
        "ast-grep project already exists.",
        "You are already inside a sub-folder of an ast-grep project. Try finding sgconfig.yml in ancestor directory?",
        PROJECT_GUIDE,
      ),
      ProjectNotExist => Self::new(
        "No ast-grep project configuration is found.",
        "You need to create an ast-grep project for this command. Try `sg new` to create one.",
        PROJECT_GUIDE,
      ),
      FileAlreadyExist(path) => Self::new(
        format!("File `{}` already exists.", path.display()),
        "The item you want to create already exists. Try editing the existing file or create a new one with a different name?",
        None,
      ),
      NoTestDirConfigured => Self::new(
        "No test file directory is configured.",
        "Fail to create a test file because the project `sgconfig.yml` does not specify any test configuration.",
        TEST_GUIDE,
      ),
      NoUtilDirConfigured => Self::new(
        "No util file directory is configured.",
        "Fail to create a utility rule because the project `sgconfig.yml` does not specify any utils directory.",
        UTIL_GUIDE,
      ),
      InsufficientCLIArgument(name) => Self::new(
        "Insufficient command line argument provided to use `--yes` option.",
        format!("You need to provide `{name}` in command line to use non-interactive `new`."),
        None,
      ),
      CannotInferShell => Self::new(
        "Can not infer which shell to generate completions.",
        "Either specify shell flavor by `sg completions [SHELL]` or set correct `SHELL` environment.",
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

// use raw ansi escape code to render links in terminal. references:
// https://gist.github.com/egmontkob/eb114294efbcd5adb1944c9f3cb5feda
// https://github.com/zkat/miette/blob/c25676cb1f4266c2607836e6359f15b9cbd8637e/src/handlers/graphical.rs#L186
fn ansi_link(url: String) -> String {
  format!(
    "\u{1b}]8;;{}\u{1b}\\{}\u{1b}]8;;\u{1b}\\",
    url,
    ansi_term::Color::Cyan.italic().paint(&url)
  )
}

struct ErrorFormat<'a> {
  context: &'a ErrorContext,
  inner: &'a Error,
}

#[derive(Default)]
struct ErrorStyle {
  message: Style,
  error: Style,
  warning: Style,
  help: Style,
  reference: Style,
}

impl ErrorStyle {
  fn colored() -> Self {
    Self {
      message: Style::new().bold(),
      error: Style::new().fg(Color::Red),
      warning: Style::new().fg(Color::Yellow),
      help: Style::new().fg(Color::Blue),
      reference: Style::new().bold().dimmed(),
    }
  }
}

impl fmt::Display for ErrorFormat<'_> {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    let ErrorMessage {
      title,
      description,
      link,
    } = ErrorMessage::from_context(self.context);
    let needs_color = std::io::stderr().is_tty();
    let style = if needs_color {
      ErrorStyle::colored()
    } else {
      ErrorStyle::default()
    };
    let (notice_style, notice, sign) = if self.context.is_soft_error() {
      (style.warning, "Warning:", "⚠")
    } else {
      (style.error, "Error:", "✖")
    };
    let message = style.message.paint(title);
    writeln!(f, "{} {message}", notice_style.paint(notice))?;
    let help = style.help.paint("Help:");
    writeln!(f, "{help} {description}")?;
    if let Some(url) = link {
      let reference = style.reference.paint("See also:");
      let link = format!("{DOC_SITE_HOST}{url}");
      let link = if needs_color { ansi_link(link) } else { link };
      writeln!(f, "{reference} {link}")?;
    }

    // skip root error
    let mut causes = self.inner.chain().skip(1).peekable();
    if causes.peek().is_none() {
      return Ok(());
    }
    writeln!(f)?;
    writeln!(f, "{} Caused by", notice_style.paint(sign))?;
    for err in causes {
      let prefix = notice_style.paint("╰▻");
      writeln!(f, "{prefix} {err}")?;
    }
    Ok(())
  }
}

#[cfg(test)]
mod test {
  use super::*;

  #[test]
  fn test_display_error() {
    let error = anyhow::anyhow!("test error").context(ErrorContext::ReadConfiguration);
    let error_fmt = ErrorFormat {
      context: &ErrorContext::ReadConfiguration,
      inner: &error,
    };
    let display = format!("{error_fmt}");
    assert_eq!(display.lines().count(), 6);
    assert!(display.contains("Cannot read configuration."));
    assert!(
      display.contains("Caused by"),
      "Should display the error chain"
    );
    assert!(display.contains("test error"));
    assert!(display.contains("Error"));
    assert!(display.contains('✖'));
  }

  #[test]
  fn test_display_warning() {
    let error = anyhow::anyhow!("test error");
    let error_fmt = ErrorFormat {
      context: &ErrorContext::PatternHasError,
      inner: &error,
    };
    let display = format!("{error_fmt}");
    assert_eq!(display.lines().count(), 3);
    assert!(display.contains("Pattern contains an ERROR node"));
    assert!(display.contains("Warning"));
  }

  #[test]
  fn test_bare_anyhow() {
    let error = anyhow::anyhow!(ErrorContext::ReadConfiguration);
    let error_fmt = ErrorFormat {
      context: &ErrorContext::ReadConfiguration,
      inner: &error,
    };
    let display = format!("{error_fmt}");
    assert_eq!(display.lines().count(), 3);
    assert!(display.contains("Cannot read configuration."));
    assert!(
      !display.contains("Caused by"),
      "Should not contain error chain"
    );
  }
}
