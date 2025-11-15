use super::{Diff, NodeMatch, PrintProcessor, Printer};
use crate::lang::SgLang;
use ast_grep_config::{RuleConfig, Severity};
use clap::ValueEnum;

use anyhow::Result;
use codespan_reporting::files::SimpleFile;
use serde_sarif::sarif;
use std::io::{Stdout, Write};

use std::borrow::Cow;
use std::path::{Path, PathBuf};

#[derive(PartialEq, Eq, Clone, ValueEnum)]
#[clap(rename_all = "lower")]
pub enum Platform {
  GitHub,
  Sarif,
}

pub enum CloudOutput {
  GitHub(Vec<u8>),
  Sarif(Vec<sarif::Result>),
}

pub struct CloudPrinter<W: Write> {
  writer: W,
  platform: Platform,
  sarif_results: Vec<sarif::Result>,
}

impl<W: Write> CloudPrinter<W> {
  pub fn new(writer: W, platform: Platform) -> Self {
    Self {
      writer,
      platform,
      sarif_results: vec![],
    }
  }
}

impl CloudPrinter<Stdout> {
  pub fn stdout(platform: Platform) -> Self {
    Self::new(std::io::stdout(), platform)
  }
}

impl<W: Write> Printer for CloudPrinter<W> {
  type Processed = CloudOutput;
  type Processor = CloudProcessor;

  fn get_processor(&self) -> Self::Processor {
    CloudProcessor {
      platform: self.platform.clone(),
    }
  }

  fn process(&mut self, processed: Self::Processed) -> Result<()> {
    match processed {
      CloudOutput::GitHub(bytes) => {
        self.writer.write_all(&bytes)?;
      }
      CloudOutput::Sarif(results) => {
        self.sarif_results.extend(results);
      }
    }
    Ok(())
  }

  fn after_print(&mut self) -> Result<()> {
    if self.platform == Platform::Sarif {
      let tool_component = sarif::ToolComponent::builder().name("ast-grep").build();
      let tool = sarif::Tool::builder().driver(tool_component).build();
      let mut run = sarif::Run::builder().tool(tool).build();
      run.results = Some(self.sarif_results.clone());
      let sarif_log = sarif::Sarif::builder()
        .version(serde_json::json!(env!("CARGO_PKG_VERSION")))
        .runs(vec![run])
        .build();
      let json = serde_json::to_string_pretty(&sarif_log)?;
      writeln!(self.writer, "{}", json)?;
    }
    Ok(())
  }
}

pub struct CloudProcessor {
  platform: Platform,
}

impl PrintProcessor<CloudOutput> for CloudProcessor {
  fn print_rule(
    &self,
    matches: Vec<NodeMatch>,
    file: SimpleFile<Cow<str>, &str>,
    rule: &RuleConfig<SgLang>,
  ) -> Result<CloudOutput> {
    match self.platform {
      Platform::GitHub => {
        let mut ret = vec![];
        let path = PathBuf::from(file.name().to_string());
        for m in matches {
          print_github_rule(&mut ret, m, &path, rule)?;
        }
        Ok(CloudOutput::GitHub(ret))
      }
      Platform::Sarif => {
        let path = file.name();
        let results = matches
          .into_iter()
          .map(|nm| create_sarif_result(&nm, path, rule))
          .collect();
        Ok(CloudOutput::Sarif(results))
      }
    }
  }

  fn print_matches(&self, _m: Vec<NodeMatch>, _p: &Path) -> Result<CloudOutput> {
    unreachable!("cloud printer does not support pattern search")
  }

  fn print_diffs(&self, _d: Vec<Diff>, _p: &Path) -> Result<CloudOutput> {
    unreachable!("cloud printer does not support pattern rewrite")
  }

  fn print_rule_diffs(
    &self,
    diffs: Vec<(Diff<'_>, &RuleConfig<SgLang>)>,
    path: &Path,
  ) -> Result<CloudOutput> {
    match self.platform {
      Platform::GitHub => {
        let mut ret = vec![];
        for (diff, rule) in diffs {
          print_github_rule(&mut ret, diff.node_match, path, rule)?;
        }
        Ok(CloudOutput::GitHub(ret))
      }
      Platform::Sarif => {
        let path = path.to_string_lossy();
        let results = diffs
          .into_iter()
          .map(|(diff, rule)| {
            let ret = create_sarif_result(&diff.node_match, &path, rule);
            attach_sarif_fix(ret, &path, diff)
          })
          .collect();
        Ok(CloudOutput::Sarif(results))
      }
    }
  }
}

fn print_github_rule<W: Write>(
  writer: &mut W,
  m: NodeMatch,
  path: &Path,
  rule: &RuleConfig<SgLang>,
) -> Result<()> {
  let level = match rule.severity {
    Severity::Error => "error",
    Severity::Warning => "warning",
    Severity::Info => "notice",
    Severity::Hint => return Ok(()),
    Severity::Off => unreachable!("turned-off rule should not have match."),
  };
  let title = &rule.id;
  let name = path.display();
  let line = m.start_pos().line() + 1;
  let end_line = m.end_pos().line() + 1;
  let message = rule.get_message(&m);
  writeln!(
    writer,
    "::{level} file={name},line={line},endLine={end_line},title={title}::{message}"
  )?;
  Ok(())
}

fn severity_to_sarif_level(severity: &Severity) -> sarif::ResultLevel {
  match severity {
    Severity::Error => sarif::ResultLevel::Error,
    Severity::Warning => sarif::ResultLevel::Warning,
    Severity::Info => sarif::ResultLevel::Note,
    Severity::Hint => sarif::ResultLevel::Note,
    Severity::Off => sarif::ResultLevel::None,
  }
}

fn create_sarif_result(
  node_match: &NodeMatch,
  path: &str,
  rule: &RuleConfig<SgLang>,
) -> sarif::Result {
  let message = rule.get_message(node_match);

  // Create the location
  let start_pos = node_match.start_pos();
  let end_pos = node_match.end_pos();
  let range = node_match.range();

  let region = sarif::Region::builder()
    .start_line((start_pos.line() + 1) as i64)
    .start_column((start_pos.column(node_match) + 1) as i64)
    .end_line((end_pos.line() + 1) as i64)
    .end_column((end_pos.column(node_match) + 1) as i64)
    .byte_offset(range.start as i64)
    .byte_length((range.end - range.start) as i64)
    .snippet(
      sarif::ArtifactContent::builder()
        .text(node_match.text().to_string())
        .build(),
    )
    .build();

  // TODO: path is not URI, this impl should handle path to uri
  let physical_location = sarif::PhysicalLocation::builder()
    .artifact_location(
      sarif::ArtifactLocation::builder()
        .uri(path.to_string())
        .build(),
    )
    .region(region)
    .build();

  let location = sarif::Location::builder()
    .physical_location(physical_location)
    .build();

  let mut result = sarif::Result::builder()
    .message(sarif::Message::builder().text(message.clone()).build())
    .build();

  result.rule_id = Some(rule.id.clone());
  result.level = Some(severity_to_sarif_level(&rule.severity));
  result.locations = Some(vec![location]);
  result
}

fn attach_sarif_fix(mut result: sarif::Result, path: &str, diff: Diff<'_>) -> sarif::Result {
  let range = diff.range;
  // Add fix information if replacement is available
  let mut deleted_region = sarif::Region::builder()
    .byte_offset(range.start as i64)
    .byte_length((range.end - range.start) as i64)
    .build();

  // only add line/column info if the diff range matches node range
  // because diff range can be larger than node range when expandStart/expandEnd is used
  // TODO: support line/column for expanded range
  let node_match = diff.node_match;
  if range == node_match.range() {
    let start_pos = node_match.start_pos();
    let end_pos = node_match.end_pos();
    deleted_region.start_line = Some(start_pos.line() as i64 + 1);
    deleted_region.start_column = Some(start_pos.column(&node_match) as i64 + 1);
    deleted_region.end_line = Some(end_pos.line() as i64 + 1);
    deleted_region.end_column = Some(end_pos.column(&node_match) as i64 + 1);
  }

  let replacement = sarif::Replacement {
    deleted_region,
    inserted_content: Some(
      sarif::ArtifactContent::builder()
        .text(diff.replacement)
        .build(),
    ),
    properties: None,
  };

  let artifact_change = sarif::ArtifactChange {
    artifact_location: sarif::ArtifactLocation::builder()
      .uri(path.to_string())
      .build(),
    replacements: vec![replacement],
    properties: None,
  };

  result.fixes = Some(vec![sarif::Fix {
    description: Some(
      sarif::Message::builder()
        .text("Apply suggested fix".to_string())
        .build(),
    ),
    artifact_changes: vec![artifact_change],
    properties: None,
  }]);
  result
}

#[cfg(test)]
mod test {
  use super::*;
  use ast_grep_config::{from_yaml_string, GlobalRules};
  use ast_grep_language::{LanguageExt, SupportLang};
  use codespan_reporting::term::termcolor::Buffer;

  fn make_test_printer() -> CloudPrinter<Buffer> {
    CloudPrinter::new(Buffer::no_color(), Platform::GitHub)
  }

  fn make_sarif_test_printer() -> CloudPrinter<Buffer> {
    CloudPrinter::new(Buffer::no_color(), Platform::Sarif)
  }
  fn get_text(printer: &mut CloudPrinter<Buffer>) -> String {
    let buffer = &mut printer.writer;
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
    let mut printer = make_test_printer();
    let grep = SgLang::from(SupportLang::Tsx).ast_grep(src);
    let rule = make_rule(rule_str);
    let matches = grep.root().find_all(&rule.matcher).collect();
    let file = SimpleFile::new(Cow::Borrowed("test.tsx"), src);
    let buffer = printer
      .get_processor()
      .print_rule(matches, file, &rule)
      .unwrap();
    printer.process(buffer).expect("should work");
    let actual = get_text(&mut printer);
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

  #[test]
  fn test_sarif_output() {
    let mut printer = make_sarif_test_printer();
    let source = "let a = 123";
    let grep = SgLang::from(SupportLang::Tsx).ast_grep(source);
    let rule = make_rule("rule: { pattern: a }\nseverity: error");
    let matches: Vec<_> = grep.root().find_all(&rule.matcher).collect();
    printer.before_print().unwrap();
    let file = SimpleFile::new(Cow::Borrowed("test.ts"), source);
    let buffer = printer
      .get_processor()
      .print_rule(matches, file, &rule)
      .unwrap();
    printer.process(buffer).unwrap();
    printer.after_print().unwrap();
    let json_str = get_text(&mut printer);

    // Verify it's valid JSON
    let sarif_log: sarif::Sarif = serde_json::from_str(&json_str).expect("should be valid SARIF");
    assert_eq!(
      sarif_log.version,
      serde_json::json!(env!("CARGO_PKG_VERSION"))
    );
    assert_eq!(sarif_log.runs.len(), 1);

    let run = &sarif_log.runs[0];
    let results = run.results.as_ref().unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].rule_id.as_ref().unwrap(), "test");
  }
}
