use super::{Diff, Printer};
use crate::error::ErrorContext as EC;
use crate::lang::SgLang;
use crate::utils;

use anyhow::{Context, Result};
use ast_grep_config::RuleConfig;
use ast_grep_core::{NodeMatch as SgNodeMatch, StrDoc};
use codespan_reporting::files::SimpleFile;

use std::sync::atomic::{AtomicBool, Ordering};

type NodeMatch<'a, L> = SgNodeMatch<'a, StrDoc<L>>;

use std::borrow::Cow;
use std::path::{Path, PathBuf};

// add this macro because neither trait_alias nor type_alias_impl is supported.
macro_rules! Matches {
  ($lt: lifetime) => { impl Iterator<Item = NodeMatch<$lt, SgLang>> };
}
macro_rules! Diffs {
  ($lt: lifetime) => { impl Iterator<Item = Diff<$lt>> };
}

pub struct InteractivePrinter<P: Printer> {
  accept_all: AtomicBool,
  from_stdin: bool,
  inner: P,
}

impl<P: Printer> InteractivePrinter<P> {
  pub fn new(inner: P, accept_all: bool, from_stdin: bool) -> Result<Self> {
    if from_stdin && !accept_all {
      Err(anyhow::anyhow!(EC::StdInIsNotInteractive))
    } else {
      Ok(Self {
        accept_all: AtomicBool::new(accept_all),
        from_stdin,
        inner,
      })
    }
  }

  fn prompt_edit(&self) -> char {
    const EDIT_PROMPT: &str = "Accept change? (Yes[y], No[n], Accept All[a], Quit[q], Edit[e])";
    utils::prompt(EDIT_PROMPT, "ynaqe", Some('n')).expect("Error happened during prompt")
  }

  fn prompt_view(&self) -> char {
    const VIEW_PROMPT: &str = "Next[enter], Quit[q], Edit[e]";
    utils::prompt(VIEW_PROMPT, "qe", Some('\n')).expect("cannot fail")
  }

  fn rewrite_action(&self, diffs: Vec<Diff<'_>>, path: &PathBuf) -> Result<()> {
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
  fn print_rule<'a>(
    &self,
    matches: Matches!('a),
    file: SimpleFile<Cow<str>, &String>,
    rule: &RuleConfig<SgLang>,
  ) -> Result<()> {
    utils::run_in_alternate_screen(|| {
      let matches: Vec<_> = matches.collect();
      let first_match = match matches.first() {
        Some(n) => n.start_pos().0,
        None => return Ok(()),
      };
      let file_path = PathBuf::from(file.name().to_string());
      self.inner.print_rule(matches.into_iter(), file, rule)?;
      let resp = self.prompt_view();
      if resp == 'q' {
        Err(anyhow::anyhow!("Exit interactive editing"))
      } else if resp == 'e' {
        utils::open_in_editor(&file_path, first_match)?;
        Ok(())
      } else {
        Ok(())
      }
    })
  }

  fn print_matches<'a>(&self, matches: Matches!('a), path: &Path) -> Result<()> {
    utils::run_in_alternate_screen(|| print_matches_and_confirm_next(self, matches, path))
  }

  fn print_diffs<'a>(&self, diffs: Diffs!('a), path: &Path) -> Result<()> {
    let path = path.to_path_buf();
    if self.accept_all.load(Ordering::SeqCst) {
      return self.rewrite_action(diffs.collect(), &path);
    }
    utils::run_in_alternate_screen(|| {
      let all = print_diffs_and_prompt_action(self, &path, diffs, None)?;
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
    rule: &RuleConfig<SgLang>,
  ) -> Result<()> {
    let path = path.to_path_buf();
    if self.accept_all.load(Ordering::SeqCst) {
      return self.rewrite_action(diffs.collect(), &path);
    }
    utils::run_in_alternate_screen(|| {
      let all = print_diffs_and_prompt_action(self, &path, diffs, Some(rule))?;
      if all {
        self.accept_all.store(true, Ordering::SeqCst);
      }
      Ok(())
    })
  }
}

/// returns if accept_all is chosen
fn print_diffs_and_prompt_action<'a>(
  interactive: &InteractivePrinter<impl Printer>,
  path: &PathBuf,
  diffs: Diffs!('a),
  rule: Option<&RuleConfig<SgLang>>,
) -> Result<bool> {
  let printer = &interactive.inner;
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
  match interactive.prompt_edit() {
    'y' => {
      interactive.rewrite_action(diffs, path)?;
      Ok(false)
    }
    'a' => {
      interactive.rewrite_action(diffs, path)?;
      Ok(true)
    }
    'n' => Ok(false),
    'e' => {
      utils::open_in_editor(path, first_match)?;
      Ok(false)
    }
    'q' => Err(anyhow::anyhow!("Exit interactive editing")),
    _ => Ok(false),
  }
}

fn print_matches_and_confirm_next<'a>(
  interactive: &InteractivePrinter<impl Printer>,
  matches: Matches!('a),
  path: &Path,
) -> Result<()> {
  let printer = &interactive.inner;
  let matches: Vec<_> = matches.collect();
  let first_match = match matches.first() {
    Some(n) => n.start_pos().0,
    None => return Ok(()),
  };
  printer.print_matches(matches.into_iter(), path)?;
  let resp = interactive.prompt_view();
  if resp == 'q' {
    Err(anyhow::anyhow!("Exit interactive editing"))
  } else if resp == 'e' {
    utils::open_in_editor(&path.to_path_buf(), first_match)?;
    Ok(())
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
  use ast_grep_config::{from_yaml_string, GlobalRules};
  use ast_grep_core::replacer::Fixer;
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
    fixer: &Fixer<String>,
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
    let matcher = config.matcher;
    let fixer = config.fixer.unwrap();
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
      &Fixer::try_new("$A", &SupportLang::TypeScript).expect("fixer must compile"),
    );
    let ret = apply_rewrite(diffs);
    assert_eq!("Some(1)", ret);
  }
}
