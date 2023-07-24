use super::{Diff, Printer};
use crate::lang::SgLang;
use ast_grep_config::{RuleConfig, Severity};
use clap::ValueEnum;

use anyhow::Result;
use ast_grep_core::{NodeMatch as SgNodeMatch, StrDoc};
use codespan_reporting::files::SimpleFile;
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

#[derive(PartialEq, Eq, Clone, ValueEnum)]
#[clap(rename_all = "lower")]
pub enum Platform {
  GitHub,
}

pub struct CloudPrinter<W: Write> {
  writer: Mutex<W>,
}

impl<W: Write> CloudPrinter<W> {
  pub fn new(w: W) -> Self {
    Self {
      writer: Mutex::new(w),
    }
  }
}

impl CloudPrinter<Stdout> {
  pub fn stdout() -> Self {
    Self::new(std::io::stdout())
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
  use ast_grep_language::{Language, SupportLang};
  use codespan_reporting::term::termcolor::Buffer;

  fn make_test_printer() -> CloudPrinter<Buffer> {
    CloudPrinter::new(Buffer::no_color())
  }
  fn get_text(printer: &CloudPrinter<Buffer>) -> String {
    let buffer = printer.writer.lock().expect("should work");
    let bytes = buffer.as_slice();
    std::str::from_utf8(bytes)
      .expect("buffer should be valid utf8")
      .to_owned()
  }

  fn make_rule(rule: &str) -> RuleConfig<SgLang> {
    let globals = GlobalRules::default();
    from_yaml_string(
      &format!(
        r"
id: test
message: test rule
language: TypeScript
{rule}"
      ),
      &globals,
    )
    .unwrap()
    .pop()
    .unwrap()
  }

  fn test_output(src: &str, rule_str: &str, expect: &str) {
    let src = src.to_owned();
    let printer = make_test_printer();
    let grep = SgLang::from(SupportLang::Tsx).ast_grep(&src);
    let rule = make_rule(rule_str);
    let matches = grep.root().find_all(&rule.matcher);
    let file = SimpleFile::new(Cow::Borrowed("test.tsx"), &src);
    printer.print_rule(matches, file, &rule).unwrap();
    let actual = get_text(&printer);
    assert_eq!(actual, expect);
  }

  #[test]
  fn test_no_match_output() {
    test_output("let a = 123", "rule: { pattern: console }", "");
    test_output(
      "let a = 123",
      "
rule: { pattern: console }
severity: error",
      "",
    );
  }

  #[test]
  fn test_hint_output() {
    test_output(
      "console.log(123)",
      "
rule: { pattern: console }
severity: hint
",
      "",
    );
  }

  #[test]
  fn test_info_output() {
    test_output(
      "console.log(123)",
      "
rule: { pattern: console }
severity: info
",
      "::notice file=test.tsx,line=1,endLine=1,title=test::test rule\n",
    );
  }

  #[test]
  fn test_warning_output() {
    test_output(
      "console.log(123)",
      "
rule: { pattern: console }
severity: warning
",
      "::warning file=test.tsx,line=1,endLine=1,title=test::test rule\n",
    );
  }

  #[test]
  fn test_error_output() {
    test_output(
      "console.log(123)",
      "
rule: { pattern: console }
severity: error
",
      "::error file=test.tsx,line=1,endLine=1,title=test::test rule\n",
    );
  }
}
