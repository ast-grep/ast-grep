use std::collections::HashMap;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use ast_grep_config::{RuleCollection, RuleConfig, Severity};
use ast_grep_core::{Matcher, NodeMatch, StrDoc};
use clap::Args;
use ignore::WalkParallel;

use crate::config::{find_rules, read_rule_file, register_custom_language};
use crate::error::ErrorContext as EC;
use crate::lang::SgLang;
use crate::print::{
  CloudPrinter, ColoredPrinter, Diff, InteractivePrinter, JSONPrinter, Platform, Printer,
  ReportStyle, SimpleFile,
};
use crate::utils::{filter_file_interactive, InputArgs, OutputArgs};
use crate::utils::{run_std_in, StdInWorker};
use crate::utils::{run_worker, Items, Worker};

type AstGrep = ast_grep_core::AstGrep<StrDoc<SgLang>>;

#[derive(Args)]
pub struct ScanArg {
  /// Path to ast-grep root config, default is sgconfig.yml.
  #[clap(short, long, value_name = "CONFIG_FILE")]
  config: Option<PathBuf>,

  /// Scan the codebase with the single rule located at the path RULE_FILE.
  ///
  /// This flags conflicts with --config. It is useful to run single rule without project setup.
  #[clap(short, long, conflicts_with = "config", value_name = "RULE_FILE")]
  rule: Option<PathBuf>,

  /// Output warning/error messages in GitHub Action format.
  ///
  /// Currently, only GitHub is supported.
  #[clap(short, long, conflicts_with = "json", conflicts_with = "interactive")]
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
  register_custom_language(arg.config.clone());
  if let Some(_format) = &arg.format {
    let printer = CloudPrinter::stdout();
    return run_scan(arg, printer);
  }
  if let Some(json) = arg.output.json {
    let printer = JSONPrinter::stdout(json);
    return run_scan(arg, printer);
  }
  let printer = ColoredPrinter::stdout(arg.output.color).style(arg.report_style);
  let interactive = arg.output.needs_interacive();
  if interactive {
    let from_stdin = arg.input.is_stdin();
    let printer = InteractivePrinter::new(printer, arg.output.update_all, from_stdin)?;
    run_scan(arg, printer)
  } else {
    run_scan(arg, printer)
  }
}

fn run_scan<P: Printer + Sync>(arg: ScanArg, printer: P) -> Result<()> {
  if arg.input.is_stdin() {
    let worker = ScanWithRule::try_new(arg, printer)?;
    run_std_in(worker)
  } else {
    let worker = ScanWithConfig::try_new(arg, printer)?;
    run_worker(worker)
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
    } else {
      find_rules(arg.config.take())?
    };
    Ok(Self {
      arg,
      printer,
      configs,
    })
  }
}

impl<P: Printer + Sync> Worker for ScanWithConfig<P> {
  type Item = (PathBuf, AstGrep);
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
    if combined.find(&unit.grep) {
      return Some((unit.path, unit.grep));
    }
    None
  }
  fn consume_items(&self, items: Items<Self::Item>) -> Result<()> {
    self.printer.before_print()?;
    let mut has_error = 0;
    for (path, grep) in items {
      let file_content = grep.root().text().to_string();
      let path = &path;
      let rules = self.configs.for_path(path);
      let combined = CombinedScan::new(rules);
      let matched = combined.scan(&grep);
      for (idx, matches) in matched {
        let rule = &combined.rules[idx];
        if matches!(rule.severity, Severity::Error) {
          has_error += 1;
        }
        match_rule_on_file(path, matches, rule, &file_content, &self.printer)?;
      }
    }
    self.printer.after_print()?;
    if has_error > 0 {
      Err(anyhow::anyhow!(EC::DiagnosticError(has_error)))
    } else {
      Ok(())
    }
  }
}

struct ScanWithRule<Printer> {
  printer: Printer,
  rule: RuleConfig<SgLang>,
}
impl<P: Printer> ScanWithRule<P> {
  fn try_new(arg: ScanArg, printer: P) -> Result<Self> {
    let rule = if let Some(path) = &arg.rule {
      read_rule_file(path, None)?.pop().unwrap()
    } else {
      return Err(anyhow::anyhow!(EC::RuleNotSpecified));
    };
    Ok(Self { printer, rule })
  }
}

impl<P: Printer + Sync> Worker for ScanWithRule<P> {
  type Item = (PathBuf, AstGrep);
  fn build_walk(&self) -> WalkParallel {
    unreachable!()
  }
  fn produce_item(&self, _p: &Path) -> Option<Self::Item> {
    unreachable!()
  }
  fn consume_items(&self, items: Items<Self::Item>) -> Result<()> {
    self.printer.before_print()?;
    let mut has_error = 0;
    for (path, grep) in items {
      let file_content = grep.root().text().to_string();
      let rule = &self.rule;
      let matches = grep.root().find_all(&rule.matcher).collect();
      if matches!(rule.severity, Severity::Error) {
        has_error += 1;
      }
      match_rule_on_file(&path, matches, rule, &file_content, &self.printer)?;
    }
    self.printer.after_print()?;
    if has_error > 0 {
      Err(anyhow::anyhow!(EC::DiagnosticError(has_error)))
    } else {
      Ok(())
    }
  }
}

impl<P: Printer + Sync> StdInWorker for ScanWithRule<P> {
  fn parse_stdin(&self, src: String) -> Option<Self::Item> {
    use ast_grep_core::Language;
    let lang = self.rule.language;
    let grep = lang.ast_grep(src);
    let has_match = grep.root().find(&self.rule.matcher).is_some();
    has_match.then(|| (PathBuf::from("STDIN"), grep))
  }
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
  if let Some(fixer) = &rule.fixer {
    let diffs = matches.map(|m| Diff::generate(m, &rule.matcher, fixer));
    reporter.print_rule_diffs(diffs, path, rule)?;
  } else {
    reporter.print_rule(matches, file, rule)?;
  }
  Ok(())
}

struct CombinedScan<'r> {
  rules: Vec<&'r RuleConfig<SgLang>>,
  kind_rule_mapping: Vec<Vec<usize>>,
}

impl<'r> CombinedScan<'r> {
  fn new(rules: Vec<&'r RuleConfig<SgLang>>) -> Self {
    let mut mapping = Vec::new();
    for (idx, rule) in rules.iter().enumerate() {
      for kind in &rule
        .matcher
        .potential_kinds()
        .unwrap_or_else(|| panic!("rule `{}` must have kind", &rule.id))
      {
        // NOTE: common languages usually have about several hundred kinds
        // from 200+ ~ 500+, it is okay to waste about 500 * 24 Byte vec size = 12kB
        // see https://github.com/Wilfred/difftastic/tree/master/vendored_parsers
        while mapping.len() <= kind {
          mapping.push(vec![]);
        }
        mapping[kind].push(idx);
      }
    }
    Self {
      rules,
      kind_rule_mapping: mapping,
    }
  }

  fn find(&self, root: &AstGrep) -> bool {
    for node in root.root().dfs() {
      let kind = node.kind_id() as usize;
      let Some(rule_idx) = self.kind_rule_mapping.get(kind) else {
        continue;
      };
      for &idx in rule_idx {
        let rule = &self.rules[idx];
        if rule.matcher.match_node(node.clone()).is_some() {
          return true;
        }
      }
    }
    false
  }
  fn scan<'a>(&self, root: &'a AstGrep) -> HashMap<usize, Vec<NodeMatch<'a, StrDoc<SgLang>>>> {
    let mut results = HashMap::new();
    for node in root.root().dfs() {
      let kind = node.kind_id() as usize;
      let Some(rule_idx) = self.kind_rule_mapping.get(kind) else {
        continue;
      };
      for &idx in rule_idx {
        let rule = &self.rules[idx];
        let Some(ret) = rule.matcher.match_node(node.clone()) else {
          continue;
        };
        let matches = results.entry(idx).or_insert_with(Vec::new);
        matches.push(ret);
      }
    }
    results
  }
}

#[cfg(test)]
mod test {
  use super::*;
  use crate::print::ColorArg;
  use std::fs::File;
  use std::io::Write;
  use tempdir::TempDir;

  const RULE: &str = r#"
id: test
message: Add your rule message here....
severity: error # error, warning, hint, info
language: Rust
rule:
  pattern: Some(123)
"#;

  pub fn create_test_files<'a>(
    names_and_contents: impl IntoIterator<Item = (&'a str, &'a str)>,
  ) -> TempDir {
    let dir = TempDir::new("sgtest").unwrap();
    for (name, contents) in names_and_contents {
      let path = dir.path().join(name);
      let mut file = File::create(path.clone()).unwrap();
      file.write_all(contents.as_bytes()).unwrap();
      file.sync_all().unwrap();
    }
    dir
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
      rule: None,
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
    };
    assert!(run_with_config(arg).is_ok());
  }
}
