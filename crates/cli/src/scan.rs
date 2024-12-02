use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use ast_grep_config::{
  from_yaml_string, CombinedScan, PreScan, RuleCollection, RuleConfig, SerializableRule,
  SerializableRuleConfig, SerializableRuleCore, Severity,
};
use ast_grep_core::{NodeMatch, StrDoc};
use clap::Args;
use ignore::WalkParallel;

use crate::config::{read_rule_file, with_rule_stats, ProjectConfig};
use crate::lang::SgLang;
use crate::print::{
  CloudPrinter, ColoredPrinter, Diff, InteractivePrinter, JSONPrinter, Platform, Printer,
  ReportStyle, SimpleFile,
};
use crate::utils::ErrorContext as EC;
use crate::utils::RuleOverwrite;
use crate::utils::{filter_file_interactive, ContextArgs, InputArgs, OutputArgs, OverwriteArgs};
use crate::utils::{FileTrace, ScanTrace};
use crate::utils::{Items, PathWorker, StdInWorker, Worker};

use std::collections::HashSet;

type AstGrep = ast_grep_core::AstGrep<StrDoc<SgLang>>;

#[derive(Args)]
pub struct ScanArg {
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

  /// Output warning/error messages in GitHub Action format.
  ///
  /// Currently, only GitHub is supported.
  #[clap(long, conflicts_with = "json", conflicts_with = "interactive")]
  format: Option<Platform>,

  #[clap(long, default_value = "rich", conflicts_with = "json")]
  report_style: ReportStyle,

  /// severity related options
  #[clap(flatten)]
  overwrite: OverwriteArgs,

  /// input related options
  #[clap(flatten)]
  input: InputArgs,
  /// output related options
  #[clap(flatten)]
  output: OutputArgs,
  /// context related options
  #[clap(flatten)]
  context: ContextArgs,
}

pub fn run_with_config(arg: ScanArg, project: Result<ProjectConfig>) -> Result<()> {
  let context = arg.context.get();
  if let Some(_format) = &arg.format {
    let printer = CloudPrinter::stdout();
    return run_scan(arg, printer, project);
  }
  if let Some(json) = arg.output.json {
    let printer = JSONPrinter::stdout(json);
    return run_scan(arg, printer, project);
  }
  let printer = ColoredPrinter::stdout(arg.output.color)
    .style(arg.report_style)
    .context(context);
  let interactive = arg.output.needs_interactive();
  if interactive {
    let from_stdin = arg.input.stdin;
    let printer = InteractivePrinter::new(printer, arg.output.update_all, from_stdin)?;
    run_scan(arg, printer, project)
  } else {
    run_scan(arg, printer, project)
  }
}

fn run_scan<P: Printer + 'static>(
  arg: ScanArg,
  printer: P,
  project: Result<ProjectConfig>,
) -> Result<()> {
  if arg.input.stdin {
    let worker = ScanWithRule::try_new(arg, printer)?;
    // TODO: report a soft error if rules have different languages
    worker.run_std_in()
  } else {
    let worker = ScanWithConfig::try_new(arg, printer, project)?;
    worker.run_path()
  }
}

struct ScanWithConfig<Printer> {
  arg: ScanArg,
  printer: Printer,
  configs: RuleCollection<SgLang>,
  unused_suppression_rule: RuleConfig<SgLang>,
  trace: ScanTrace,
}
impl<P: Printer> ScanWithConfig<P> {
  fn try_new(arg: ScanArg, printer: P, project: Result<ProjectConfig>) -> Result<Self> {
    let overwrite = RuleOverwrite::new(&arg.overwrite)?;
    let unused_suppression_rule = unused_suppression_rule_config(&overwrite);
    let (configs, rule_trace) = if let Some(path) = &arg.rule {
      let rules = read_rule_file(path, None)?;
      with_rule_stats(rules)?
    } else if let Some(text) = &arg.inline_rules {
      let rules = from_yaml_string(text, &Default::default())
        .with_context(|| EC::ParseRule("INLINE_RULES".into()))?;
      with_rule_stats(rules)?
    } else {
      // NOTE: only query project here since -r does not need project
      let project_config = project?;
      project_config.find_rules(overwrite)?
    };
    let trace = arg.output.inspect.scan_trace(rule_trace);
    trace.print_rules(&configs)?;
    Ok(Self {
      arg,
      printer,
      configs,
      unused_suppression_rule,
      trace,
    })
  }
}
impl<P: Printer> Worker for ScanWithConfig<P> {
  type Item = (PathBuf, AstGrep, PreScan);
  fn consume_items(&self, items: Items<Self::Item>) -> Result<()> {
    self.printer.before_print()?;
    let mut error_count = 0usize;
    for (path, grep, pre_scan) in items {
      let file_content = grep.source().to_string();
      let path = &path;
      let rules = self.configs.get_rule_from_lang(path, *grep.lang());
      let combined = CombinedScan::new(rules);
      let interactive = self.arg.output.needs_interactive();
      // exclude_fix rule because we already have diff inspection before
      let scanned = combined.scan(&grep, pre_scan, /* separate_fix*/ interactive);
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
      error_count += print_unused_suppressions(
        path,
        scanned.unused_suppressions,
        &self.unused_suppression_rule,
        &file_content,
        &self.printer,
      )?;
    }
    self.printer.after_print()?;
    self.trace.print()?;
    if error_count > 0 {
      Err(anyhow::anyhow!(EC::DiagnosticError(error_count)))
    } else {
      Ok(())
    }
  }
}

fn unused_suppression_rule_config(overwrite: &RuleOverwrite) -> RuleConfig<SgLang> {
  let rule: SerializableRule = serde_json::from_str(r#"{"pattern": "a"}"#).unwrap();
  let core = SerializableRuleCore {
    rule,
    constraints: None,
    fix: None,
    transform: None,
    utils: None,
  };
  let severity = overwrite
    .find("unused-suppression")
    .severity
    .unwrap_or(Severity::Hint);
  let config = SerializableRuleConfig::<SgLang> {
    core,
    id: "unused-suppression".to_string(),
    severity,
    files: None,
    ignores: None,
    language: "rust".parse().unwrap(),
    message: "Unused 'ast-grep-ignore' directive.".into(),
    metadata: None,
    note: None,
    rewriters: None,
    url: None,
  };
  RuleConfig::try_from(config, &Default::default()).unwrap()
}

fn print_unused_suppressions(
  path: &Path,
  matches: Vec<NodeMatch<StrDoc<SgLang>>>,
  rule_config: &RuleConfig<SgLang>,
  file_content: &String,
  printer: &impl Printer,
) -> Result<usize> {
  let count = match rule_config.severity {
    Severity::Error => matches.len(),
    // skip printing turned-off rule
    Severity::Off => return Ok(0),
    _ => 0,
  };
  match_rule_on_file(path, matches, rule_config, file_content, printer)?;
  Ok(count)
}

impl<P: Printer> PathWorker for ScanWithConfig<P> {
  fn get_trace(&self) -> &FileTrace {
    &self.trace.file_trace
  }
  fn build_walk(&self) -> Result<WalkParallel> {
    let mut langs = HashSet::new();
    self.configs.for_each_rule(|rule| {
      langs.insert(rule.language);
    });
    self.arg.input.walk_langs(langs.into_iter())
  }
  fn produce_item(&self, path: &Path) -> Option<Vec<Self::Item>> {
    filter_file_interactive(path, &self.configs, &self.trace)
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
  type Item = (PathBuf, AstGrep, PreScan);
  fn consume_items(&self, items: Items<Self::Item>) -> Result<()> {
    self.printer.before_print()?;
    let mut error_count = 0usize;
    let combined = CombinedScan::new(self.rules.iter().collect());
    for (path, grep, pre_scan) in items {
      let file_content = grep.source().to_string();
      // do not exclude_fix rule in run_with_rule
      let scanned = combined.scan(&grep, pre_scan, false);
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
    let pre_scan = combined.find(&grep);
    if !pre_scan.is_empty() {
      Some((PathBuf::from("STDIN"), grep, pre_scan))
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
      rule: None,
      inline_rules: None,
      report_style: ReportStyle::Rich,
      input: InputArgs {
        no_ignore: vec![],
        paths: vec![PathBuf::from(".")],
        stdin: false,
        follow: false,
        globs: vec![],
        threads: 0,
      },
      overwrite: OverwriteArgs {
        filter: None,
        error: None,
        warning: None,
        info: None,
        hint: None,
        off: None,
      },
      output: OutputArgs {
        interactive: false,
        json: None,
        update_all: false,
        color: ColorArg::Never,
        inspect: Default::default(),
      },
      context: ContextArgs {
        before: 0,
        after: 0,
        context: 0,
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
    let project_config = ProjectConfig::setup(Some(dir.path().join("sgconfig.yml"))).unwrap();
    let arg = default_scan_arg();
    assert!(run_with_config(arg, project_config).is_ok());
  }

  #[test]
  fn test_scan_with_inline_rules() {
    let inline_rules = "{id: test, language: ts, rule: {pattern: readFileSync}}".to_string();
    let arg = ScanArg {
      inline_rules: Some(inline_rules),
      ..default_scan_arg()
    };
    assert!(run_with_config(arg, Err(anyhow::anyhow!("not found"))).is_ok());
  }

  #[test]
  fn test_scan_with_inline_rules_diff() {
    let inline_rules =
      "{id: test, language: ts, rule: {pattern: readFileSync}, fix: 'nnn'}".to_string();
    let arg = ScanArg {
      inline_rules: Some(inline_rules),
      ..default_scan_arg()
    };
    assert!(run_with_config(arg, Err(anyhow::anyhow!("not found"))).is_ok());
  }

  // baseline test for coverage
  #[test]
  fn test_scan_with_inline_rules_error() {
    let inline_rules = "nonsense".to_string();
    let arg = ScanArg {
      inline_rules: Some(inline_rules),
      ..default_scan_arg()
    };
    let err = run_with_config(arg, Err(anyhow::anyhow!("not found"))).expect_err("should error");
    assert!(err.is::<EC>());
    assert_eq!(err.to_string(), "Cannot parse rule INLINE_RULES");
  }
}
