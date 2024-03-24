use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use ast_grep_config::{from_yaml_string, CombinedScan, RuleCollection, RuleConfig, Severity};
use ast_grep_core::{NodeMatch, StrDoc};
use bit_set::BitSet;
use clap::Args;
use ignore::WalkParallel;
use regex::Regex;

use crate::config::{find_rules, read_rule_file, register_custom_language};
use crate::error::ErrorContext as EC;
use crate::lang::SgLang;
use crate::print::{
  CloudPrinter, ColoredPrinter, Diff, InteractivePrinter, JSONPrinter, Platform, Printer,
  ReportStyle, SimpleFile,
};
use crate::utils::{filter_file_interactive, InputArgs, OutputArgs};
use crate::utils::{Items, PathWorker, StdInWorker, Worker};

type AstGrep = ast_grep_core::AstGrep<StrDoc<SgLang>>;

#[derive(Args)]
pub struct ScanArg {
  /// Path to ast-grep root config, default is sgconfig.yml.
  #[clap(short, long, value_name = "CONFIG_FILE")]
  config: Option<PathBuf>,

  /// Scan the codebase with the single rule located at the path RULE_FILE.
  ///
  /// It is useful to run single rule without project setup or sgconfig.yml.
  #[clap(short, long, value_name = "RULE_FILE")]
  rule: Option<PathBuf>,

  /// Scan the codebase with a rule defined by the provided RULE_TEXT.
  ///
  /// Use this argument if you want to test a rule without creating a YAML file on disk.
  /// You can run multiple rules by separating them with `---` in the RULE_TEXT.
  /// --inline-rules is incompatible with --rule.
  #[clap(long, conflicts_with = "rule", value_name = "RULE_TEXT")]
  inline_rules: Option<String>,

  /// Scan the codebase with rules with ids matching REGEX.
  ///
  /// This flags conflicts with --rule. It is useful to scan with a subset of rules from a large
  /// set of rule definitions within a project.
  #[clap(long, conflicts_with = "rule", value_name = "REGEX")]
  filter: Option<Regex>,

  /// Output warning/error messages in GitHub Action format.
  ///
  /// Currently, only GitHub is supported.
  #[clap(long, conflicts_with = "json", conflicts_with = "interactive")]
  format: Option<Platform>,

  #[clap(long, default_value = "rich", conflicts_with = "json")]
  report_style: ReportStyle,

  /// input related options
  #[clap(flatten)]
  input: InputArgs,
  /// output related options
  #[clap(flatten)]
  output: OutputArgs,
}

pub fn run_with_config(arg: ScanArg) -> Result<()> {
  register_custom_language(arg.config.clone())?;
  if let Some(_format) = &arg.format {
    let printer = CloudPrinter::stdout();
    return run_scan(arg, printer);
  }
  if let Some(json) = arg.output.json {
    let printer = JSONPrinter::stdout(json);
    return run_scan(arg, printer);
  }
  let printer = ColoredPrinter::stdout(arg.output.color).style(arg.report_style);
  let interactive = arg.output.needs_interactive();
  if interactive {
    let from_stdin = arg.input.stdin;
    let printer = InteractivePrinter::new(printer, arg.output.update_all, from_stdin)?;
    run_scan(arg, printer)
  } else {
    run_scan(arg, printer)
  }
}

fn run_scan<P: Printer + 'static>(arg: ScanArg, printer: P) -> Result<()> {
  if arg.input.stdin {
    let worker = ScanWithRule::try_new(arg, printer)?;
    // TODO: report a soft error if rules have different languages
    worker.run_std_in()
  } else {
    let worker = ScanWithConfig::try_new(arg, printer)?;
    worker.run_path()
  }
}

struct ScanWithConfig<Printer> {
  arg: ScanArg,
  printer: Printer,
  configs: RuleCollection<SgLang>,
}
impl<P: Printer> ScanWithConfig<P> {
  fn try_new(mut arg: ScanArg, printer: P) -> Result<Self> {
    let configs = if let Some(path) = &arg.rule {
      let rules = read_rule_file(path, None)?;
      RuleCollection::try_new(rules).context(EC::GlobPattern)?
    } else if let Some(text) = &arg.inline_rules {
      let rules = from_yaml_string(text, &Default::default())
        .with_context(|| EC::ParseRule("INLINE_RULES".into()))?;
      RuleCollection::try_new(rules).context(EC::GlobPattern)?
    } else {
      find_rules(arg.config.take(), arg.filter.as_ref())?
    };
    Ok(Self {
      arg,
      printer,
      configs,
    })
  }
}
impl<P: Printer> Worker for ScanWithConfig<P> {
  type Item = (PathBuf, AstGrep, BitSet);
  fn consume_items(&self, items: Items<Self::Item>) -> Result<()> {
    self.printer.before_print()?;
    let mut error_count = 0usize;
    for (path, grep, hit_set) in items {
      let file_content = grep.source().to_string();
      let path = &path;
      let rules = self.configs.for_path(path);
      let combined = CombinedScan::new(rules);
      let interactive = self.arg.output.needs_interactive();
      // exclude_fix rule because we already have diff inspection before
      let scanned = combined.scan(&grep, hit_set, /* separate_fix*/ interactive);
      if interactive {
        let diffs = scanned
          .diffs
          .into_iter()
          .map(|(idx, nm)| {
            let rule = combined.get_rule(idx);
            (nm, rule)
          })
          .collect();
        match_rule_diff_on_file(path, diffs, &self.printer)?;
      }
      for (idx, matches) in scanned.matches {
        let rule = combined.get_rule(idx);
        if matches!(rule.severity, Severity::Error) {
          error_count = error_count.saturating_add(matches.len());
        }
        match_rule_on_file(path, matches, rule, &file_content, &self.printer)?;
      }
    }
    self.printer.after_print()?;
    if error_count > 0 {
      Err(anyhow::anyhow!(EC::DiagnosticError(error_count)))
    } else {
      Ok(())
    }
  }
}

impl<P: Printer> PathWorker for ScanWithConfig<P> {
  fn build_walk(&self) -> WalkParallel {
    self.arg.input.walk()
  }
  fn produce_item(&self, path: &Path) -> Option<Self::Item> {
    let rules = self.configs.for_path(path);
    if rules.is_empty() {
      return None;
    }
    let lang = rules[0].language;
    let combined = CombinedScan::new(rules);
    let unit = filter_file_interactive(path, lang, ast_grep_core::matcher::MatchAll)?;
    let hit_set = combined.find(&unit.grep);
    if !hit_set.is_empty() {
      return Some((unit.path, unit.grep, hit_set));
    }
    None
  }
}

struct ScanWithRule<Printer> {
  printer: Printer,
  rules: Vec<RuleConfig<SgLang>>,
}
impl<P: Printer> ScanWithRule<P> {
  fn try_new(arg: ScanArg, printer: P) -> Result<Self> {
    let rules = if let Some(path) = &arg.rule {
      read_rule_file(path, None)?
    } else if let Some(text) = &arg.inline_rules {
      from_yaml_string(text, &Default::default())
        .with_context(|| EC::ParseRule("INLINE_RULES".into()))?
    } else {
      return Err(anyhow::anyhow!(EC::RuleNotSpecified));
    };
    Ok(Self { printer, rules })
  }
}

impl<P: Printer> Worker for ScanWithRule<P> {
  type Item = (PathBuf, AstGrep, BitSet);
  fn consume_items(&self, items: Items<Self::Item>) -> Result<()> {
    self.printer.before_print()?;
    let mut error_count = 0usize;
    let combined = CombinedScan::new(self.rules.iter().collect());
    for (path, grep, hit_set) in items {
      let file_content = grep.source().to_string();
      // do not exclude_fix rule in run_with_rule
      let scanned = combined.scan(&grep, hit_set, false);
      for (idx, matches) in scanned.matches {
        let rule = combined.get_rule(idx);
        if matches!(rule.severity, Severity::Error) {
          error_count = error_count.saturating_add(matches.len());
        }
        match_rule_on_file(&path, matches, rule, &file_content, &self.printer)?;
      }
    }
    self.printer.after_print()?;
    if error_count > 0 {
      Err(anyhow::anyhow!(EC::DiagnosticError(error_count)))
    } else {
      Ok(())
    }
  }
}

impl<P: Printer> StdInWorker for ScanWithRule<P> {
  fn parse_stdin(&self, src: String) -> Option<Self::Item> {
    use ast_grep_core::Language;
    let lang = self.rules[0].language;
    let combined = CombinedScan::new(self.rules.iter().collect());
    let grep = lang.ast_grep(src);
    let hit_set = combined.find(&grep);
    if !hit_set.is_empty() {
      Some((PathBuf::from("STDIN"), grep, hit_set))
    } else {
      None
    }
  }
}
fn match_rule_diff_on_file(
  path: &Path,
  matches: Vec<(NodeMatch<StrDoc<SgLang>>, &RuleConfig<SgLang>)>,
  reporter: &impl Printer,
) -> Result<()> {
  let diffs = matches
    .into_iter()
    .filter_map(|(m, rule)| {
      let fix = rule.matcher.fixer.as_ref()?;
      let diff = Diff::generate(m, &rule.matcher, fix);
      Some((diff, rule))
    })
    .collect();
  reporter.print_rule_diffs(diffs, path)?;
  Ok(())
}

fn match_rule_on_file(
  path: &Path,
  matches: Vec<NodeMatch<StrDoc<SgLang>>>,
  rule: &RuleConfig<SgLang>,
  file_content: &String,
  reporter: &impl Printer,
) -> Result<()> {
  let matches = matches.into_iter();
  let file = SimpleFile::new(path.to_string_lossy(), file_content);
  if let Some(fixer) = &rule.matcher.fixer {
    let diffs = matches
      .map(|m| (Diff::generate(m, &rule.matcher, fixer), rule))
      .collect();
    reporter.print_rule_diffs(diffs, path)?;
  } else {
    reporter.print_rule(matches, file, rule)?;
  }
  Ok(())
}

#[cfg(test)]
mod test {
  use super::*;
  use crate::print::ColorArg;
  use std::fs::File;
  use std::io::Write;
  use tempfile::TempDir;

  const RULE: &str = r#"
id: test
message: Add your rule message here....
severity: error # error, warning, hint, info
language: Rust
rule:
  pattern: Some(123)
"#;

  // TODO: unify with verify::test
  pub fn create_test_files<'a>(
    names_and_contents: impl IntoIterator<Item = (&'a str, &'a str)>,
  ) -> TempDir {
    let dir = TempDir::new().unwrap();
    for (name, contents) in names_and_contents {
      let path = dir.path().join(name);
      let mut file = File::create(path.clone()).unwrap();
      file.write_all(contents.as_bytes()).unwrap();
      file.sync_all().unwrap();
    }
    dir
  }

  fn default_scan_arg() -> ScanArg {
    ScanArg {
      config: None,
      filter: None,
      rule: None,
      inline_rules: None,
      report_style: ReportStyle::Rich,
      input: InputArgs {
        no_ignore: vec![],
        paths: vec![PathBuf::from(".")],
        stdin: false,
      },
      output: OutputArgs {
        interactive: false,
        json: None,
        update_all: false,
        color: ColorArg::Never,
      },
      format: None,
    }
  }

  #[test]
  fn test_run_with_config() {
    let dir = create_test_files([("sgconfig.yml", "ruleDirs: [rules]")]);
    std::fs::create_dir_all(dir.path().join("rules")).unwrap();
    let mut file = File::create(dir.path().join("rules/test.yml")).unwrap();
    file.write_all(RULE.as_bytes()).unwrap();
    let mut file = File::create(dir.path().join("test.rs")).unwrap();
    file
      .write_all("fn test() { Some(123) }".as_bytes())
      .unwrap();
    file.sync_all().unwrap();
    let arg = ScanArg {
      config: Some(dir.path().join("sgconfig.yml")),
      ..default_scan_arg()
    };
    assert!(run_with_config(arg).is_ok());
  }

  #[test]
  fn test_scan_with_inline_rules() {
    let inline_rules = "{id: test, language: ts, rule: {pattern: console.log($A)}}".to_string();
    let arg = ScanArg {
      inline_rules: Some(inline_rules),
      ..default_scan_arg()
    };
    assert!(run_with_config(arg).is_ok());
  }

  // baseline test for coverage
  #[test]
  fn test_scan_with_inline_rules_error() {
    let inline_rules = "nonsense".to_string();
    let arg = ScanArg {
      inline_rules: Some(inline_rules),
      ..default_scan_arg()
    };
    let err = run_with_config(arg).expect_err("should error");
    assert!(err.is::<EC>());
    assert_eq!(err.to_string(), "Cannot parse rule INLINE_RULES");
  }
}
