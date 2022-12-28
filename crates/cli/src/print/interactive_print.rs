use std::sync::atomic::{AtomicBool, Ordering};

use anyhow::{Context, Result};
use ast_grep_config::RuleConfig;
use ast_grep_core::{AstGrep, Matcher, Pattern};

use super::{ColoredPrinter, Diff, Printer};
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

static ACCEPT_ALL: AtomicBool = AtomicBool::new(false);

pub struct InteractivePrinter {
  accept_all: bool,
  inner: ColoredPrinter,
}
impl InteractivePrinter {
  pub fn new() -> Self {
    Self {
      accept_all: false,
      inner: ColoredPrinter::color(codespan_reporting::term::termcolor::ColorChoice::Auto),
    }
  }
}

impl Printer for InteractivePrinter {
  fn before_print(&self) {
    ACCEPT_ALL.store(self.accept_all, Ordering::SeqCst);
  }
  fn print_rule<'a>(
    &self,
    matches: Matches!('a),
    file: SimpleFile<Cow<str>, &String>,
    rule: &RuleConfig<SupportLang>,
  ) {
    interaction::run_in_alternate_screen(|| {
      self.inner.print_rule(matches, file, rule);
      let resp = interaction::prompt(VIEW_PROMPT, "q", Some('\n')).expect("cannot fail");
      if resp == 'q' {
        Err(anyhow::anyhow!("Exit interactive editing"))
      } else {
        Ok(())
      }
    })
    .unwrap();
  }

  fn print_matches<'a>(&self, matches: Matches!('a), path: &Path) -> Result<()> {
    interaction::run_in_alternate_screen(|| {
      print_matches_and_confirm_next(&self.inner, matches, path)
    })
  }

  fn print_diffs<'a>(&self, diffs: Diffs!('a), path: &Path) -> Result<()> {
    interaction::run_in_alternate_screen(|| {
      print_diffs_and_prompt_action(&self.inner, &path.to_path_buf(), diffs)
    })
  }
  fn print_rule_diffs<'a>(
    &self,
    diffs: Diffs!('a),
    path: &Path,
    _rule: &RuleConfig<SupportLang>,
  ) -> Result<()> {
    self.print_diffs(diffs, path)
  }
}

const EDIT_PROMPT: &str = "Accept change? (Yes[y], No[n], Accept All[a], Quit[q], Edit[e])";
const VIEW_PROMPT: &str = "Next[enter], Quit[q]";

fn print_diffs_and_prompt_action<'a>(
  printer: &impl Printer,
  path: &PathBuf,
  diffs: Diffs!('a),
) -> Result<()> {
  let diffs: Vec<_> = diffs.collect();
  let rewrite_action = || {
    let new_content = apply_rewrite(diffs.clone().into_iter());
    std::fs::write(path, new_content).with_context(|| EC::WriteFile(path.clone()))?;
    Ok(())
  };
  if ACCEPT_ALL.load(Ordering::SeqCst) {
    return rewrite_action();
  }
  let first_match = match diffs.first() {
    Some(n) => n.node_match.start_pos().0,
    None => return Ok(()),
  };
  printer.print_diffs(diffs.clone().into_iter(), path)?;
  let response =
    interaction::prompt(EDIT_PROMPT, "ynaqe", Some('n')).expect("Error happened during prompt");
  match response {
    'y' => rewrite_action(),
    'a' => {
      ACCEPT_ALL.store(true, Ordering::SeqCst);
      rewrite_action()
    }
    'n' => Ok(()),
    'e' => interaction::open_in_editor(path, first_match),
    'q' => Err(anyhow::anyhow!("Exit interactive editing")),
    _ => Ok(()),
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

fn apply_rewrite<'a>(diffs: Diffs!('a)) -> String {
  todo!()
}

#[cfg(test)]
mod test {
  use super::*;
  use ast_grep_config::from_yaml_string;

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

  // #[test]
  // fn test_apply_rewrite() {
  //   let root = AstGrep::new("let a = () => c++", SupportLang::TypeScript);
  //   let config = make_rule(
  //     r"
  // rule:
  // all:
  //   - pattern: $B
  //   - any:
  //       - pattern: $A++
  // fix: ($B, lifecycle.update(['$A']))",
  //   );
  //   let ret = apply_rewrite(&root, config.get_matcher(), &config.get_fixer().unwrap());
  //   assert_eq!(ret, "let a = () => (c++, lifecycle.update(['c']))");
  // }

  // #[test]
  // fn test_rewrite_nested() {
  //   let root = SupportLang::TypeScript.ast_grep("Some(Some(1))");
  //   let ret = apply_rewrite(
  //     &root,
  //     "Some($A)",
  //     &Pattern::new("$A", SupportLang::TypeScript),
  //   );
  //   assert_eq!("Some(1)", ret);
  // }
}
