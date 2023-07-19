use super::{Diff, Printer};
use crate::lang::SgLang;
use ast_grep_config::{RuleConfig, Severity};
use clap::ValueEnum;

use anyhow::Result;
use ast_grep_core::{NodeMatch as SgNodeMatch, StrDoc};
pub use codespan_reporting::{files::SimpleFile, term::ColorArg};
use std::io::{Stdout, Write};
use std::sync::Mutex;

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

#[derive(PartialEq, Eq, Clone, Default, ValueEnum)]
#[clap(rename_all = "lower")]
pub enum Platform {
  #[default]
  Local,
  GitHub,
}

pub struct CloudPrinter<W: Write> {
  writer: Mutex<W>,
}

impl CloudPrinter<Stdout> {
  pub fn stdout() -> Self {
    Self {
      writer: Mutex::new(std::io::stdout()),
    }
  }
}

impl<W: Write> Printer for CloudPrinter<W> {
  fn print_rule<'a>(
    &self,
    matches: Matches!('a),
    file: SimpleFile<Cow<str>, &String>,
    rule: &RuleConfig<SgLang>,
  ) -> Result<()> {
    let path = PathBuf::from(file.name().to_string());
    print_rule(self, matches, &path, rule)
  }

  fn print_matches<'a>(&self, _m: Matches!('a), _p: &Path) -> Result<()> {
    unreachable!()
  }

  fn print_diffs<'a>(&self, _d: Diffs!('a), _p: &Path) -> Result<()> {
    unreachable!()
  }

  fn print_rule_diffs<'a>(
    &self,
    diffs: Diffs!('a),
    path: &Path,
    rule: &RuleConfig<SgLang>,
  ) -> Result<()> {
    print_rule(self, diffs.map(|d| d.node_match), path, rule)
  }
}

fn print_rule<'a, W: Write>(
  p: &CloudPrinter<W>,
  matches: Matches!('a),
  path: &Path,
  rule: &RuleConfig<SgLang>,
) -> Result<()> {
  let mut writer = p.writer.lock().expect("should work");
  let level = match rule.severity {
    Severity::Error => "error",
    Severity::Warning => "warning",
    Severity::Info => "notice",
    Severity::Hint => return Ok(()),
    Severity::Off => unreachable!("turned-off rule should not have match."),
  };
  let title = &rule.id;
  let name = path.display();
  for m in matches {
    let line = m.start_pos().0 + 1;
    let end_line = m.end_pos().0 + 1;
    let message = rule.get_message(&m);
    writeln!(
      &mut writer,
      "::{level} file={name},line={line},endLine={end_line},title={title}::{message}"
    )?;
  }
  Ok(())
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
}
