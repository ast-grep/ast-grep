use super::{Diff, NodeMatch, PrintProcessor, Printer};
use crate::lang::SgLang;
use ast_grep_config::{RuleConfig, Severity};

use anyhow::Result;
use codespan_reporting::files::SimpleFile;
use serde_sarif::sarif;
use std::io::{Stdout, Write};

use std::borrow::Cow;
use std::path::Path;

pub struct SarifPrinter<W: Write> {
  output: W,
  runs: Vec<sarif::Run>,
}

impl SarifPrinter<Stdout> {
  pub fn stdout() -> Self {
    Self::new(std::io::stdout())
  }
}

impl<W: Write> SarifPrinter<W> {
  pub fn new(output: W) -> Self {
    Self {
      output,
      runs: vec![],
    }
  }
}

impl<W: Write> Printer for SarifPrinter<W> {
  type Processed = SarifResult;
  type Processor = SarifProcessor;

  fn get_processor(&self) -> Self::Processor {
    SarifProcessor
  }

  fn process(&mut self, processed: Self::Processed) -> Result<()> {
    if processed.results.is_empty() {
      return Ok(());
    }
    
    // Merge results into the first run or create a new one
    if self.runs.is_empty() {
      let tool_component = sarif::ToolComponent::builder()
        .name("ast-grep")
        .build();
      let tool = sarif::Tool::builder()
        .driver(tool_component)
        .build();
      let mut run = sarif::Run::builder()
        .tool(tool)
        .build();
      run.results = Some(processed.results);
      self.runs.push(run);
    } else {
      let run = &mut self.runs[0];
      if let Some(results) = &mut run.results {
        results.extend(processed.results);
      } else {
        run.results = Some(processed.results);
      }
    }
    Ok(())
  }

  fn before_print(&mut self) -> Result<()> {
    Ok(())
  }

  fn after_print(&mut self) -> Result<()> {
    let sarif_log = sarif::Sarif::builder()
      .version(serde_json::json!("2.1.0"))
      .runs(self.runs.clone())
      .build();
    let json = serde_json::to_string_pretty(&sarif_log)?;
    writeln!(self.output, "{}", json)?;
    Ok(())
  }
}

pub struct SarifProcessor;

pub struct SarifResult {
  results: Vec<sarif::Result>,
}

impl PrintProcessor<SarifResult> for SarifProcessor {
  fn print_rule(
    &self,
    matches: Vec<NodeMatch>,
    file: SimpleFile<Cow<str>, &str>,
    rule: &RuleConfig<SgLang>,
  ) -> Result<SarifResult> {
    let path = file.name();
    let results = matches
      .into_iter()
      .map(|nm| create_sarif_result(&nm, path, rule, None))
      .collect();
    Ok(SarifResult { results })
  }

  fn print_matches(&self, _matches: Vec<NodeMatch>, _path: &Path) -> Result<SarifResult> {
    // SARIF is designed for rule-based analysis, not pattern matching
    Ok(SarifResult {
      results: vec![],
    })
  }

  fn print_diffs(&self, _diffs: Vec<Diff>, _path: &Path) -> Result<SarifResult> {
    // SARIF doesn't directly support diffs without rules
    Ok(SarifResult {
      results: vec![],
    })
  }

  fn print_rule_diffs(
    &self,
    diffs: Vec<(Diff, &RuleConfig<SgLang>)>,
    path: &Path,
  ) -> Result<SarifResult> {
    let path = path.to_string_lossy();
    let results = diffs
      .into_iter()
      .map(|(diff, rule)| create_sarif_result(&diff.node_match, &path, rule, Some(diff.replacement)))
      .collect();
    Ok(SarifResult { results })
  }
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
  replacement: Option<String>,
) -> sarif::Result {
  let message = rule.get_message(node_match);
  
  // Create the location
  let start_pos = node_match.start_pos();
  let end_pos = node_match.end_pos();
  
  let region = sarif::Region::builder()
    .start_line((start_pos.line() + 1) as i64)
    .start_column((start_pos.column(node_match) + 1) as i64)
    .end_line((end_pos.line() + 1) as i64)
    .end_column((end_pos.column(node_match) + 1) as i64)
    .byte_offset(node_match.range().start as i64)
    .byte_length((node_match.range().end - node_match.range().start) as i64)
    .snippet(sarif::ArtifactContent::builder()
      .text(node_match.text().to_string())
      .build())
    .build();
  
  let physical_location = sarif::PhysicalLocation::builder()
    .artifact_location(sarif::ArtifactLocation::builder()
      .uri(path.to_string())
      .build())
    .region(region)
    .build();
  
  let location = sarif::Location::builder()
    .physical_location(physical_location)
    .build();
  
  let mut result = sarif::Result::builder()
    .message(sarif::Message::builder()
      .text(message.clone())
      .build())
    .build();
  
  result.rule_id = Some(rule.id.clone());
  result.level = Some(severity_to_sarif_level(&rule.severity));
  result.locations = Some(vec![location]);
  
  // Add fix information if replacement is available
  if let Some(replacement_text) = replacement {
    let deleted_region = sarif::Region::builder()
      .start_line((start_pos.line() + 1) as i64)
      .start_column((start_pos.column(node_match) + 1) as i64)
      .end_line((end_pos.line() + 1) as i64)
      .end_column((end_pos.column(node_match) + 1) as i64)
      .byte_offset(node_match.range().start as i64)
      .byte_length((node_match.range().end - node_match.range().start) as i64)
      .build();
    
    let replacement = sarif::Replacement {
      deleted_region,
      inserted_content: Some(sarif::ArtifactContent::builder()
        .text(replacement_text)
        .build()),
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
      description: Some(sarif::Message::builder()
        .text("Apply suggested fix".to_string())
        .build()),
      artifact_changes: vec![artifact_change],
      properties: None,
    }]);
  }
  
  result
}

#[cfg(test)]
mod test {
  use super::*;
  use ast_grep_config::{from_yaml_string, GlobalRules};
  use ast_grep_language::{LanguageExt, SupportLang};

  struct Test(String);
  impl Write for Test {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
      let s = std::str::from_utf8(buf).expect("should ok");
      self.0.push_str(s);
      Ok(buf.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
      Ok(())
    }
  }

  fn make_test_printer() -> SarifPrinter<Test> {
    SarifPrinter::new(Test(String::new()))
  }

  fn get_text(printer: &SarifPrinter<Test>) -> String {
    let output = &printer.output;
    output.0.to_string()
  }

  fn make_rule(rule: &str) -> RuleConfig<SgLang> {
    let globals = GlobalRules::default();
    from_yaml_string(
      &format!(
        r#"
id: test
message: test rule
severity: error
language: TypeScript
rule:
  pattern: {rule}"#
      ),
      &globals,
    )
    .unwrap()
    .pop()
    .unwrap()
  }

  #[test]
  fn test_sarif_output() {
    let mut printer = make_test_printer();
    let source = "let a = 123";
    let grep = SgLang::from(SupportLang::Tsx).ast_grep(source);
    let rule = make_rule("a");
    let matches: Vec<_> = grep.root().find_all(&rule.matcher).collect();
    printer.before_print().unwrap();
    let file = SimpleFile::new(Cow::Borrowed("test.ts"), source);
    let buffer = printer
      .get_processor()
      .print_rule(matches, file, &rule)
      .unwrap();
    printer.process(buffer).unwrap();
    printer.after_print().unwrap();
    let json_str = get_text(&printer);
    
    // Verify it's valid JSON
    let sarif_log: sarif::Sarif = serde_json::from_str(&json_str).expect("should be valid SARIF");
    assert_eq!(sarif_log.version, serde_json::json!("2.1.0"));
    assert_eq!(sarif_log.runs.len(), 1);
    
    let run = &sarif_log.runs[0];
    let results = run.results.as_ref().unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].rule_id.as_ref().unwrap(), "test");
  }
}
