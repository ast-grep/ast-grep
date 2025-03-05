use super::{Diff, NodeMatch, PrintProcessor, Printer};
use crate::lang::SgLang;
use crate::utils;
use crate::utils::ErrorContext as EC;

use anyhow::{Context, Result};
use ast_grep_config::RuleConfig;
use codespan_reporting::files::SimpleFile;

use std::borrow::Cow;
use std::ops::Range;
use std::path::{Path, PathBuf};

pub struct InteractivePrinter<P: Printer> {
  accept_all: bool,
  from_stdin: bool,
  committed_cnt: usize,
  inner: P,
}

impl<P: Printer> InteractivePrinter<P> {
  pub fn new(inner: P, accept_all: bool, from_stdin: bool) -> Result<Self> {
    if from_stdin && !accept_all {
      Err(anyhow::anyhow!(EC::StdInIsNotInteractive))
    } else {
      Ok(Self {
        accept_all,
        from_stdin,
        inner,
        committed_cnt: 0,
      })
    }
  }

  fn prompt_edit(&self) -> char {
    if self.accept_all {
      return 'a';
    }
    const EDIT_PROMPT: &str = "Accept change? (Yes[y], No[n], Accept All[a], Quit[q], Edit[e])";
    utils::prompt(EDIT_PROMPT, "ynaqe", Some('n')).expect("Error happened during prompt")
  }

  fn prompt_view(&self) -> char {
    if self.accept_all {
      return '\n';
    }
    const VIEW_PROMPT: &str = "Next[enter], Quit[q], Edit[e]";
    utils::prompt(VIEW_PROMPT, "qe", Some('\n')).expect("cannot fail")
  }

  fn rewrite_action(&self, diffs: Diffs<()>, path: &PathBuf) -> Result<()> {
    if diffs.contents.is_empty() {
      return Ok(());
    }
    let new_content = apply_rewrite(diffs);
    if self.from_stdin {
      println!("{new_content}");
      Ok(())
    } else {
      std::fs::write(path, new_content).with_context(|| EC::WriteFile(path.clone()))
    }
  }

  fn process_highlights(&mut self, highlights: Highlights<P::Processed>) -> Result<()> {
    let Highlights {
      path,
      first_line,
      inner,
    } = highlights;
    utils::run_in_alternate_screen(|| {
      self.inner.process(inner)?;
      let resp = self.prompt_view();
      if resp == 'q' {
        Err(anyhow::anyhow!("Exit interactive editing"))
      } else if resp == 'e' {
        open_in_editor(&path, first_line)?;
        Ok(())
      } else {
        Ok(())
      }
    })
  }

  fn process_diffs(&mut self, diffs: Diffs<P::Processed>) -> Result<()> {
    let path = diffs.path.clone();
    let (confirmed, all) = process_diffs_interactive(self, diffs)?;
    self.rewrite_action(confirmed, &path)?;
    if all {
      self.accept_all = true;
    }
    Ok(())
  }
}

impl<P> Printer for InteractivePrinter<P>
where
  P: Printer + 'static,
  P::Processor: Send + 'static,
{
  type Processed = Payload<P>;
  type Processor = InteractiveProcessor<P>;

  fn get_processor(&self) -> Self::Processor {
    InteractiveProcessor {
      inner: self.inner.get_processor(),
    }
  }

  fn process(&mut self, processed: Self::Processed) -> Result<()> {
    use InteractivePayload as IP;
    match processed {
      IP::Nothing => Ok(()),
      IP::Highlights(h) => self.process_highlights(h),
      IP::Diffs(d) => self.process_diffs(d),
    }
  }

  fn after_print(&mut self) -> Result<()> {
    if self.committed_cnt > 0 {
      println!("Applied {} changes", self.committed_cnt);
    }
    self.inner.after_print()
  }
}

pub struct InteractiveDiff<D> {
  /// string content for the replacement
  replacement: String,
  range: Range<usize>,
  first_line: usize,
  display: D,
}

impl<D> InteractiveDiff<D> {
  fn new(diff: Diff, display: D) -> Self {
    Self {
      first_line: diff.node_match.start_pos().line(),
      replacement: diff.replacement,
      range: diff.range,
      display,
    }
  }
}

pub struct Highlights<D> {
  path: PathBuf,
  first_line: usize,
  inner: D,
}

pub struct Diffs<D> {
  path: PathBuf,
  // TODO: this clone is slow
  old_source: String,
  contents: Vec<InteractiveDiff<D>>,
}

pub enum InteractivePayload<D> {
  Nothing,
  Highlights(Highlights<D>),
  Diffs(Diffs<D>),
}

pub struct InteractiveProcessor<P: Printer> {
  inner: P::Processor,
}

pub type Payload<P> = InteractivePayload<<P as Printer>::Processed>;

impl<P> PrintProcessor<Payload<P>> for InteractiveProcessor<P>
where
  P: Printer + 'static,
{
  fn print_rule(
    &self,
    matches: Vec<NodeMatch>,
    file: SimpleFile<Cow<str>, &String>,
    rule: &RuleConfig<SgLang>,
  ) -> Result<Payload<P>> {
    let Some(first_match) = matches.first() else {
      return Ok(InteractivePayload::Nothing);
    };
    let first_line = first_match.start_pos().line();
    let path = PathBuf::from(file.name().to_string());
    let inner = self.inner.print_rule(matches, file, rule)?;
    let highlights = Highlights {
      inner,
      first_line,
      path,
    };
    Ok(InteractivePayload::Highlights(highlights))
  }

  fn print_matches(&self, matches: Vec<NodeMatch>, path: &Path) -> Result<Payload<P>> {
    let Some(first_match) = matches.first() else {
      return Ok(InteractivePayload::Nothing);
    };
    let first_line = first_match.start_pos().line();
    let inner = self.inner.print_matches(matches, path)?;
    let highlights = Highlights {
      inner,
      first_line,
      path: path.to_path_buf(),
    };
    Ok(InteractivePayload::Highlights(highlights))
  }

  fn print_diffs(&self, diffs: Vec<Diff>, path: &Path) -> Result<Payload<P>> {
    let old_source = get_old_source(diffs.first());
    let mut contents = Vec::with_capacity(diffs.len());
    for diff in diffs {
      let display = self.inner.print_diffs(vec![diff.clone()], path)?;
      let content = InteractiveDiff::new(diff, display);
      contents.push(content);
    }
    Ok(InteractivePayload::Diffs(Diffs {
      path: path.to_path_buf(),
      old_source,
      contents,
    }))
  }
  fn print_rule_diffs(
    &self,
    diffs: Vec<(Diff<'_>, &RuleConfig<SgLang>)>,
    path: &Path,
  ) -> Result<Payload<P>> {
    let old_source = get_old_source(diffs.first().map(|d| &d.0));
    let mut contents = Vec::with_capacity(diffs.len());
    for (diff, rule) in diffs {
      let display = self
        .inner
        .print_rule_diffs(vec![(diff.clone(), rule)], path)?;
      let content = InteractiveDiff::new(diff, display);
      contents.push(content);
    }
    Ok(InteractivePayload::Diffs(Diffs {
      path: path.to_path_buf(),
      old_source,
      contents,
    }))
  }
}

fn get_old_source(diff: Option<&Diff>) -> String {
  let Some(node) = diff else {
    return String::new();
  };
  node.get_root_text().to_string()
}

fn process_diffs_interactive<P: Printer>(
  interactive: &mut InteractivePrinter<P>,
  diffs: Diffs<P::Processed>,
) -> Result<(Diffs<()>, bool)> {
  let mut confirmed = vec![];
  let mut all = interactive.accept_all;
  let mut end = 0;
  let path = diffs.path;
  for diff in diffs.contents {
    if diff.range.start < end {
      continue;
    }
    let to_confirm = InteractiveDiff {
      first_line: diff.first_line,
      range: diff.range.clone(),
      replacement: diff.replacement.clone(),
      display: (),
    };
    let confirm = all || {
      let (accept_curr, accept_all) = print_diff_and_prompt_action(interactive, &path, diff)?;
      all = accept_all;
      accept_curr
    };
    if confirm {
      end = to_confirm.range.end;
      confirmed.push(to_confirm);
      interactive.committed_cnt = interactive.committed_cnt.saturating_add(1);
    }
  }
  let diffs = Diffs {
    path,
    old_source: diffs.old_source,
    contents: confirmed,
  };
  Ok((diffs, all))
}
/// returns if accept_current and accept_all
fn print_diff_and_prompt_action<P: Printer>(
  interactive: &mut InteractivePrinter<P>,
  path: &Path,
  processed: InteractiveDiff<P::Processed>,
) -> Result<(bool, bool)> {
  utils::run_in_alternate_screen(|| {
    let printer = &mut interactive.inner;
    printer.process(processed.display)?;
    match interactive.prompt_edit() {
      'y' => Ok((true, false)),
      'a' => Ok((true, true)),
      'e' => {
        let pos = processed.first_line;
        open_in_editor(path, pos)?;
        Ok((false, false))
      }
      'q' => Err(anyhow::anyhow!("Exit interactive editing")),
      'n' => Ok((false, false)),
      _ => Ok((false, false)),
    }
  })
}

fn apply_rewrite(diffs: Diffs<()>) -> String {
  let mut new_content = String::new();
  let old_content = diffs.old_source;
  let mut start = 0;
  for diff in diffs.contents {
    let range = diff.range;
    new_content.push_str(&old_content[start..range.start]);
    new_content.push_str(&diff.replacement);
    start = range.end;
  }
  // add trailing statements
  new_content.push_str(&old_content[start..]);
  new_content
}

/// start_line is zero-based
fn open_in_editor(path: &Path, start_line: usize) -> Result<()> {
  let editor = std::env::var("EDITOR").unwrap_or_else(|_| String::from("vim"));
  let exit = std::process::Command::new(editor)
    .arg(path)
    .arg(format!("+{}", start_line + 1))
    .spawn()
    .context(EC::OpenEditor)?
    .wait()
    .context(EC::OpenEditor)?;
  if exit.success() {
    Ok(())
  } else {
    Err(anyhow::anyhow!(EC::OpenEditor))
  }
}

#[cfg(test)]
mod test {
  use super::*;
  use ast_grep_config::{from_yaml_string, Fixer, GlobalRules};
  use ast_grep_core::traversal::Visitor;
  use ast_grep_core::{AstGrep, Matcher, StrDoc};
  use ast_grep_language::SupportLang;

  fn make_rule(rule: &str) -> RuleConfig<SgLang> {
    let globals = GlobalRules::default();
    from_yaml_string(
      &format!(
        r"
id: test
message: test rule
severity: info
language: TypeScript
{rule}"
      ),
      &globals,
    )
    .unwrap()
    .pop()
    .unwrap()
  }

  fn make_diffs(
    grep: &AstGrep<StrDoc<SgLang>>,
    matcher: impl Matcher<SgLang>,
    fixer: &Fixer<SgLang>,
  ) -> Diffs<()> {
    let root = grep.root();
    let old_source = root.root().get_text().to_string();
    let contents = Visitor::new(&matcher)
      .reentrant(false)
      .visit(root)
      .map(|nm| {
        let diff = Diff::generate(nm, &matcher, fixer);
        InteractiveDiff {
          first_line: 0,
          range: diff.range,
          replacement: diff.replacement,
          display: (),
        }
      })
      .collect();
    Diffs {
      old_source,
      path: PathBuf::new(),
      contents,
    }
  }

  #[test]
  fn test_apply_rewrite() {
    let root = AstGrep::new("let a = () => c++", SupportLang::TypeScript.into());
    let config = make_rule(
      r"
rule:
  all:
    - pattern: $B
    - any:
        - pattern: $A++
fix: ($B, lifecycle.update(['$A']))",
    );
    let mut matcher = config.matcher;
    let fixer = matcher.fixer.take().unwrap();
    let diffs = make_diffs(&root, matcher, &fixer);
    let ret = apply_rewrite(diffs);
    assert_eq!(ret, "let a = () => (c++, lifecycle.update(['c']))");
  }

  #[test]
  fn test_rewrite_nested() {
    let root = AstGrep::new("Some(Some(1))", SupportLang::TypeScript.into());
    let diffs = make_diffs(
      &root,
      "Some($A)",
      &Fixer::from_str("$A", &SupportLang::TypeScript.into()).expect("fixer must compile"),
    );
    let ret = apply_rewrite(diffs);
    assert_eq!("Some(1)", ret);
  }

  // https://github.com/ast-grep/ast-grep/issues/668
  #[test]
  fn test_rewrite_with_empty_lines() {
    let root = AstGrep::new("\n\n\nSome(1)", SupportLang::TypeScript.into());
    let diffs = make_diffs(
      &root,
      "Some($A)",
      &Fixer::from_str("$A", &SupportLang::TypeScript.into()).expect("fixer must compile"),
    );
    let ret = apply_rewrite(diffs);
    assert_eq!("\n\n\n1", ret);
  }

  fn test_open_editor_respect_editor_env() {
    std::env::set_var("EDITOR", "echo");
    let exit = open_in_editor(&PathBuf::from("Cargo.toml"), 1);
    assert!(exit.is_ok());
  }

  fn test_open_editor_error_handling() {
    std::env::set_var("EDITOR", "NOT_EXIST_XXXXX");
    let exit = open_in_editor(&PathBuf::from("Cargo.toml"), 1);
    let error = exit.expect_err("should be error");
    let error = error.downcast_ref::<EC>().expect("should be error context");
    assert!(matches!(error, EC::OpenEditor));
  }

  #[test]
  fn test_open_editor() {
    // these two tests must run in sequence
    // since setting env will cause racing condition
    test_open_editor_respect_editor_env();
    test_open_editor_error_handling();
  }
}
