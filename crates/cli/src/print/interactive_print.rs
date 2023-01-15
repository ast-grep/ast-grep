use std::sync::atomic::{AtomicBool, Ordering};

use anyhow::{Context, Result};
use ast_grep_config::RuleConfig;

use super::{Diff, Printer};
use crate::error::ErrorContext as EC;
use crate::interaction;
use ast_grep_core::NodeMatch;
use ast_grep_language::SupportLang;

pub use codespan_reporting::{files::SimpleFile, term::ColorArg};

use std::borrow::Cow;
use std::path::{Path, PathBuf};

// add this macro because neither trait_alias nor type_alias_impl is supported.
macro_rules! Matches {
  ($lt: lifetime) => { impl Iterator<Item = NodeMatch<$lt, SupportLang>> };
}
macro_rules! Diffs {
  ($lt: lifetime) => { impl Iterator<Item = Diff<$lt>> };
}

pub struct InteractivePrinter<P: Printer> {
  accept_all: AtomicBool,
  inner: P,
}
impl<P: Printer> InteractivePrinter<P> {
  pub fn new(inner: P) -> Self {
    Self {
      accept_all: AtomicBool::new(false),
      inner,
    }
  }

  pub fn accept_all(self, accept_all: bool) -> Self {
    self.accept_all.store(accept_all, Ordering::SeqCst);
    self
  }
}

impl<P: Printer> Printer for InteractivePrinter<P> {
  fn print_rule<'a>(
    &self,
    matches: Matches!('a),
    file: SimpleFile<Cow<str>, &String>,
    rule: &RuleConfig<SupportLang>,
  ) -> Result<()> {
    interaction::run_in_alternate_screen(|| {
      self.inner.print_rule(matches, file, rule)?;
      let resp = interaction::prompt(VIEW_PROMPT, "q", Some('\n')).expect("cannot fail");
      if resp == 'q' {
        Err(anyhow::anyhow!("Exit interactive editing"))
      } else {
        Ok(())
      }
    })
  }

  fn print_matches<'a>(&self, matches: Matches!('a), path: &Path) -> Result<()> {
    interaction::run_in_alternate_screen(|| {
      print_matches_and_confirm_next(&self.inner, matches, path)
    })
  }

  fn print_diffs<'a>(&self, diffs: Diffs!('a), path: &Path) -> Result<()> {
    let path = path.to_path_buf();
    if self.accept_all.load(Ordering::SeqCst) {
      return rewrite_action(diffs.collect(), &path);
    }
    interaction::run_in_alternate_screen(|| {
      let all = print_diffs_and_prompt_action(&self.inner, &path, diffs, None)?;
      if all {
        self.accept_all.store(true, Ordering::SeqCst);
      }
      Ok(())
    })
  }
  fn print_rule_diffs<'a>(
    &self,
    diffs: Diffs!('a),
    path: &Path,
    rule: &RuleConfig<SupportLang>,
  ) -> Result<()> {
    let path = path.to_path_buf();
    if self.accept_all.load(Ordering::SeqCst) {
      return rewrite_action(diffs.collect(), &path);
    }
    interaction::run_in_alternate_screen(|| {
      let all = print_diffs_and_prompt_action(&self.inner, &path, diffs, Some(rule))?;
      if all {
        self.accept_all.store(true, Ordering::SeqCst);
      }
      Ok(())
    })
  }
}

const EDIT_PROMPT: &str = "Accept change? (Yes[y], No[n], Accept All[a], Quit[q], Edit[e])";
const VIEW_PROMPT: &str = "Next[enter], Quit[q]";

fn rewrite_action(diffs: Vec<Diff<'_>>, path: &PathBuf) -> Result<()> {
  let new_content = apply_rewrite(diffs);
  std::fs::write(path, new_content).with_context(|| EC::WriteFile(path.clone()))
}

/// returns if accept_all is chosen
fn print_diffs_and_prompt_action<'a>(
  printer: &impl Printer,
  path: &PathBuf,
  diffs: Diffs!('a),
  rule: Option<&RuleConfig<SupportLang>>,
) -> Result<bool> {
  let diffs: Vec<_> = diffs.collect();
  let first_match = match diffs.first() {
    Some(n) => n.node_match.start_pos().0,
    None => return Ok(false),
  };
  if let Some(rule) = rule {
    printer.print_rule_diffs(diffs.clone().into_iter(), path, rule)?;
  } else {
    printer.print_diffs(diffs.clone().into_iter(), path)?;
  }
  let response =
    interaction::prompt(EDIT_PROMPT, "ynaqe", Some('n')).expect("Error happened during prompt");
  match response {
    'y' => {
      rewrite_action(diffs, path)?;
      Ok(false)
    }
    'a' => {
      rewrite_action(diffs, path)?;
      Ok(true)
    }
    'n' => Ok(false),
    'e' => {
      interaction::open_in_editor(path, first_match)?;
      Ok(false)
    }
    'q' => Err(anyhow::anyhow!("Exit interactive editing")),
    _ => Ok(false),
  }
}

fn print_matches_and_confirm_next<'a>(
  printer: &impl Printer,
  matches: Matches!('a),
  path: &Path,
) -> Result<()> {
  printer.print_matches(matches, path)?;
  let resp = interaction::prompt(VIEW_PROMPT, "q", Some('\n')).expect("cannot fail");
  if resp == 'q' {
    Err(anyhow::anyhow!("Exit interactive editing"))
  } else {
    Ok(())
  }
}

fn apply_rewrite(diffs: Vec<Diff>) -> String {
  let mut new_content = String::new();
  let Some(first) = diffs.first() else {
    return new_content
  };
  let old_content = first.node_match.ancestors().last().unwrap().text();
  let mut start = 0;
  for diff in diffs {
    let range = diff.node_match.range();
    new_content.push_str(&old_content[start..range.start]);
    new_content.push_str(&diff.replacement);
    start = range.end;
  }
  // add trailing statements
  new_content.push_str(&old_content[start..]);
  new_content
}

#[cfg(test)]
mod test {
  use super::*;
  use ast_grep_config::from_yaml_string;
  use ast_grep_core::traversal::Visitor;
  use ast_grep_core::{AstGrep, Matcher, Pattern};

  fn make_rule(rule: &str) -> RuleConfig<SupportLang> {
    from_yaml_string(&format!(
      r"
id: test
message: test rule
severity: info
language: TypeScript
{rule}"
    ))
    .unwrap()
    .pop()
    .unwrap()
  }

  fn make_diffs<'a>(
    grep: &'a AstGrep<SupportLang>,
    matcher: impl Matcher<SupportLang>,
    fixer: &Pattern<SupportLang>,
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
    let root = AstGrep::new("let a = () => c++", SupportLang::TypeScript);
    let config = make_rule(
      r"
rule:
  all:
    - pattern: $B
    - any:
        - pattern: $A++
fix: ($B, lifecycle.update(['$A']))",
    );
    let matcher = config.matcher;
    let fixer = config.fixer.unwrap();
    let diffs = make_diffs(&root, matcher, &fixer);
    let ret = apply_rewrite(diffs);
    assert_eq!(ret, "let a = () => (c++, lifecycle.update(['c']))");
  }

  #[test]
  fn test_rewrite_nested() {
    let root = AstGrep::new("Some(Some(1))", SupportLang::TypeScript);
    let diffs = make_diffs(
      &root,
      "Some($A)",
      &Pattern::new("$A", SupportLang::TypeScript),
    );
    let ret = apply_rewrite(diffs);
    assert_eq!("Some(1)", ret);
  }
}
