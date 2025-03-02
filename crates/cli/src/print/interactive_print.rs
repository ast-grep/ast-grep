use super::{Diff, NodeMatch, Printer};
use crate::lang::SgLang;
use crate::utils;
use crate::utils::ErrorContext as EC;

use anyhow::{Context, Result};
use ast_grep_config::RuleConfig;
use codespan_reporting::files::SimpleFile;

use std::borrow::Cow;
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

  fn rewrite_action(&self, diffs: Vec<Diff<'_>>, path: &PathBuf) -> Result<()> {
    if diffs.is_empty() {
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
}

impl<P: Printer> Printer for InteractivePrinter<P> {
  fn print_rule(
    &mut self,
    matches: Vec<NodeMatch>,
    file: SimpleFile<Cow<str>, &String>,
    rule: &RuleConfig<SgLang>,
  ) -> Result<()> {
    utils::run_in_alternate_screen(|| {
      let first_match = match matches.first() {
        Some(n) => n.start_pos().line(),
        None => return Ok(()),
      };
      let file_path = PathBuf::from(file.name().to_string());
      self.inner.print_rule(matches, file, rule)?;
      let resp = self.prompt_view();
      if resp == 'q' {
        Err(anyhow::anyhow!("Exit interactive editing"))
      } else if resp == 'e' {
        open_in_editor(&file_path, first_match)?;
        Ok(())
      } else {
        Ok(())
      }
    })
  }

  fn print_matches(&mut self, matches: Vec<NodeMatch>, path: &Path) -> Result<()> {
    utils::run_in_alternate_screen(|| print_matches_and_confirm_next(self, matches, path))
  }

  fn print_diffs(&mut self, diffs: Vec<Diff>, path: &Path) -> Result<()> {
    let path = path.to_path_buf();
    let diffs = diffs.into_iter().map(|d| (d, None)).collect();
    let (confirmed, all) = print_diffs_interactive(self, &path, diffs)?;
    self.rewrite_action(confirmed, &path)?;
    if all {
      self.accept_all = true;
      // self.accept_all.store(true, Ordering::SeqCst);
    }
    Ok(())
  }
  fn print_rule_diffs(
    &mut self,
    diffs: Vec<(Diff<'_>, &RuleConfig<SgLang>)>,
    path: &Path,
  ) -> Result<()> {
    let path = path.to_path_buf();
    let (confirmed, all) = print_diffs_interactive(
      self,
      &path,
      diffs.into_iter().map(|(d, r)| (d, Some(r))).collect(),
    )?;
    self.rewrite_action(confirmed, &path)?;
    if all {
      self.accept_all = true;
      // self.accept_all.store(true, Ordering::SeqCst);
    }
    Ok(())
  }

  fn after_print(&mut self) -> Result<()> {
    if self.committed_cnt > 0 {
      println!("Applied {} changes", self.committed_cnt);
    }
    self.inner.after_print()
  }
}

fn print_diffs_interactive<'a>(
  interactive: &mut InteractivePrinter<impl Printer>,
  path: &Path,
  diffs: Vec<(Diff<'a>, Option<&RuleConfig<SgLang>>)>,
) -> Result<(Vec<Diff<'a>>, bool)> {
  let mut confirmed = vec![];
  let mut all = interactive.accept_all;
  let mut end = 0;
  for (diff, rule) in diffs {
    if diff.range.start < end {
      continue;
    }
    let confirm = all || {
      let (accept_curr, accept_all) =
        print_diff_and_prompt_action(interactive, path, (diff.clone(), rule))?;
      all = accept_all;
      accept_curr
    };
    if confirm {
      end = diff.range.end;
      confirmed.push(diff);
      interactive.committed_cnt = interactive.committed_cnt.saturating_add(1);
    }
  }
  Ok((confirmed, all))
}
/// returns if accept_current and accept_all
fn print_diff_and_prompt_action(
  interactive: &mut InteractivePrinter<impl Printer>,
  path: &Path,
  (diff, rule): (Diff, Option<&RuleConfig<SgLang>>),
) -> Result<(bool, bool)> {
  utils::run_in_alternate_screen(|| {
    let printer = &mut interactive.inner;
    if let Some(rule) = rule {
      printer.print_rule_diffs(vec![(diff.clone(), rule)], path)?;
    } else {
      printer.print_diffs(vec![diff.clone()], path)?;
    }
    match interactive.prompt_edit() {
      'y' => Ok((true, false)),
      'a' => Ok((true, true)),
      'e' => {
        let pos = diff.node_match.start_pos().line();
        open_in_editor(path, pos)?;
        Ok((false, false))
      }
      'q' => Err(anyhow::anyhow!("Exit interactive editing")),
      'n' => Ok((false, false)),
      _ => Ok((false, false)),
    }
  })
}

fn print_matches_and_confirm_next(
  interactive: &mut InteractivePrinter<impl Printer>,
  matches: Vec<NodeMatch>,
  path: &Path,
) -> Result<()> {
  let printer = &mut interactive.inner;
  let first_match = match matches.first() {
    Some(n) => n.start_pos().line(),
    None => return Ok(()),
  };
  printer.print_matches(matches, path)?;
  let resp = interactive.prompt_view();
  if resp == 'q' {
    Err(anyhow::anyhow!("Exit interactive editing"))
  } else if resp == 'e' {
    open_in_editor(path, first_match)?;
    Ok(())
  } else {
    Ok(())
  }
}

fn apply_rewrite(diffs: Vec<Diff>) -> String {
  let mut new_content = String::new();
  let Some(first) = diffs.first() else {
    return new_content;
  };
  let old_content = first.get_root_text();
  let mut start = 0;
  for diff in diffs {
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

  fn make_diffs<'a>(
    grep: &'a AstGrep<StrDoc<SgLang>>,
    matcher: impl Matcher<SgLang>,
    fixer: &Fixer<SgLang>,
  ) -> Vec<Diff<'a>> {
    let root = grep.root();
    Visitor::new(&matcher)
      .reentrant(false)
      .visit(root)
      .map(|nm| Diff::generate(nm, &matcher, fixer))
      .collect()
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
