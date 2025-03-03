use crate::lang::SgLang;
use ast_grep_config::{RuleConfig, Severity};
use ast_grep_core::{meta_var::MetaVariable, Node as SgNode, StrDoc};

type Node<'a, L> = SgNode<'a, StrDoc<L>>;

use std::collections::HashMap;

use super::{Diff, NodeMatch, PrintProcessor, Printer};
use anyhow::Result;
use clap::ValueEnum;
use codespan_reporting::files::SimpleFile;
use serde::{Deserialize, Serialize};

use std::borrow::Cow;
use std::io::{Stdout, Write};
use std::path::Path;

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
/// Zero-based character position in a file.
struct Position {
  /// Zero-based line number
  line: usize,
  /// Zero-based character column in a line
  column: usize,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Range {
  /// inclusive start, exclusive end
  byte_offset: std::ops::Range<usize>,
  start: Position,
  end: Position,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct LabelJSON<'a> {
  text: &'a str,
  range: Range,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct MatchNode<'a> {
  text: Cow<'a, str>,
  range: Range,
}

/// a sub field of leading and trailing text count around match.
/// plugin authors can use it to split `lines` into leading, matching and trailing
/// See ast-grep/ast-grep#1381
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CharCount {
  leading: usize,
  trailing: usize,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct MatchJSON<'a> {
  text: Cow<'a, str>,
  range: Range,
  file: Cow<'a, str>,
  lines: String,
  char_count: CharCount,
  #[serde(skip_serializing_if = "Option::is_none")]
  replacement: Option<String>,
  #[serde(skip_serializing_if = "Option::is_none")]
  replacement_offsets: Option<std::ops::Range<usize>>,
  language: SgLang,
  #[serde(skip_serializing_if = "Option::is_none")]
  meta_variables: Option<MetaVariables<'a>>,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct MetaVariables<'a> {
  single: HashMap<String, MatchNode<'a>>,
  multi: HashMap<String, Vec<MatchNode<'a>>>,
  transformed: HashMap<String, String>,
}
fn from_env<'a>(nm: &NodeMatch<'a>) -> Option<MetaVariables<'a>> {
  let env = nm.get_env();
  let mut vars = env.get_matched_variables().peekable();
  vars.peek()?;
  let mut single = HashMap::new();
  let mut multi = HashMap::new();
  let mut transformed = HashMap::new();
  for var in vars {
    use MetaVariable as MV;
    match var {
      MV::Capture(n, _) => {
        if let Some(node) = env.get_match(&n) {
          single.insert(
            n,
            MatchNode {
              text: node.text(),
              range: get_range(node),
            },
          );
        } else if let Some(bytes) = env.get_transformed(&n) {
          transformed.insert(n, String::from_utf8_lossy(bytes).into_owned());
        }
      }
      MV::MultiCapture(n) => {
        let nodes = env.get_multiple_matches(&n);
        multi.insert(
          n,
          nodes
            .into_iter()
            .map(|node| MatchNode {
              text: node.text(),
              range: get_range(&node),
            })
            .collect(),
        );
      }
      _ => continue,
    }
  }
  Some(MetaVariables {
    single,
    multi,
    transformed,
  })
}

fn get_range(n: &Node<'_, SgLang>) -> Range {
  let start_pos = n.start_pos();
  let end_pos = n.end_pos();
  Range {
    byte_offset: n.range(),
    start: Position {
      line: start_pos.line(),
      column: start_pos.column(n),
    },
    end: Position {
      line: end_pos.line(),
      column: end_pos.column(n),
    },
  }
}

impl<'a> MatchJSON<'a> {
  fn new(nm: NodeMatch<'a>, path: &'a str, context: (u16, u16)) -> Self {
    let display = nm.display_context(context.0 as usize, context.1 as usize);
    let lines = format!("{}{}{}", display.leading, display.matched, display.trailing);
    MatchJSON {
      file: Cow::Borrowed(path),
      text: nm.text(),
      lines,
      char_count: CharCount {
        leading: display.leading.chars().count(),
        trailing: display.trailing.chars().count(),
      },
      language: *nm.lang(),
      replacement: None,
      replacement_offsets: None,
      range: get_range(&nm),
      meta_variables: from_env(&nm),
    }
  }

  fn diff(diff: Diff<'a>, path: &'a str, context: (u16, u16)) -> Self {
    let mut ret = Self::new(diff.node_match, path, context);
    ret.replacement = Some(diff.replacement);
    ret.replacement_offsets = Some(diff.range);
    ret
  }
}
fn get_labels<'a>(nm: &NodeMatch<'a>) -> Option<Vec<MatchNode<'a>>> {
  let env = nm.get_env();
  let labels = env.get_labels("secondary")?;
  Some(
    labels
      .iter()
      .map(|l| MatchNode {
        text: l.text(),
        range: get_range(l),
      })
      .collect(),
  )
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RuleMatchJSON<'a> {
  #[serde(flatten)]
  matched: MatchJSON<'a>,
  rule_id: &'a str,
  severity: Severity,
  note: Option<String>,
  message: String,
  #[serde(skip_serializing_if = "Option::is_none")]
  labels: Option<Vec<MatchNode<'a>>>,
}
impl<'a> RuleMatchJSON<'a> {
  fn new(nm: NodeMatch<'a>, path: &'a str, rule: &'a RuleConfig<SgLang>) -> Self {
    let message = rule.get_message(&nm);
    let labels = get_labels(&nm);
    let matched = MatchJSON::new(nm, path, (0, 0));
    Self {
      matched,
      rule_id: &rule.id,
      severity: rule.severity.clone(),
      note: rule.note.clone(),
      message,
      labels,
    }
  }
  fn diff(diff: Diff<'a>, path: &'a str, rule: &'a RuleConfig<SgLang>) -> Self {
    let nm = &diff.node_match;
    let message = rule.get_message(nm);
    let labels = get_labels(nm);
    let matched = MatchJSON::diff(diff, path, (0, 0));
    Self {
      matched,
      rule_id: &rule.id,
      severity: rule.severity.clone(),
      note: rule.note.clone(),
      message,
      labels,
    }
  }
}

/// Controls how to print and format JSON object in output.
#[derive(Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum JsonStyle {
  /// Prints the matches as a pretty-printed JSON array, with indentation and line breaks.
  /// This is useful for human readability, but not for parsing by other programs.
  /// This is the default value for the `--json` option.
  Pretty,
  /// Prints each match as a separate JSON object, followed by a newline character.
  /// This is useful for streaming the output to other programs that can read one object per line.
  Stream,
  /// Prints the matches as a single-line JSON array, without any whitespace.
  /// This is useful for saving space and minimizing the output size.
  Compact,
}

pub struct JSONPrinter<W: Write> {
  output: W,
  style: JsonStyle,
  context: (u16, u16),
  // indicate if any matches happened
  matched: bool,
}
impl JSONPrinter<Stdout> {
  pub fn stdout(style: JsonStyle) -> Self {
    Self::new(std::io::stdout(), style)
  }
}

impl<W: Write> JSONPrinter<W> {
  pub fn new(output: W, style: JsonStyle) -> Self {
    // no match happened yet
    Self {
      style,
      output,
      context: (0, 0),
      matched: false,
    }
  }

  pub fn context(mut self, context: (u16, u16)) -> Self {
    self.context = context;
    self
  }
}

impl<W: Write> Printer for JSONPrinter<W> {
  type Processed = Buffer;
  type Processor = JSONProcessor;

  fn get_processor(&self) -> JSONProcessor {
    JSONProcessor {
      style: self.style,
      context: self.context,
    }
  }
  fn process(&mut self, processed: Buffer) -> Result<()> {
    if processed.is_empty() {
      return Ok(());
    }
    let output = &mut self.output;
    let matched = self.matched;
    self.matched = true;
    // print separator if there was a match before
    if matched {
      let separator = match self.style {
        JsonStyle::Pretty => ",\n",
        JsonStyle::Stream => "\n",
        JsonStyle::Compact => ",",
      };
      write!(output, "{separator}")?;
    } else if self.style == JsonStyle::Pretty {
      // print newline for the first match in pretty style
      writeln!(output)?;
    }
    output.write_all(&processed)?;
    Ok(())
  }

  fn before_print(&mut self) -> Result<()> {
    if self.style == JsonStyle::Stream {
      return Ok(());
    }
    write!(self.output, "[")?;
    Ok(())
  }

  fn after_print(&mut self) -> Result<()> {
    if self.style == JsonStyle::Stream {
      return Ok(());
    }
    let output = &mut self.output;
    if self.matched && self.style == JsonStyle::Pretty {
      writeln!(output)?;
    }
    writeln!(output, "]")?;
    Ok(())
  }
}

pub struct JSONProcessor {
  style: JsonStyle,
  context: (u16, u16),
}

impl JSONProcessor {
  fn print_docs<S: Serialize>(&self, mut docs: impl Iterator<Item = S>) -> Result<Buffer> {
    let mut ret = Vec::new();
    let Some(doc) = docs.next() else {
      return Ok(ret);
    };
    let output = &mut ret;
    match self.style {
      JsonStyle::Pretty => {
        serde_json::to_writer_pretty(&mut *output, &doc)?;
        for doc in docs {
          writeln!(&mut *output, ",")?;
          serde_json::to_writer_pretty(&mut *output, &doc)?;
        }
      }
      JsonStyle::Stream => {
        serde_json::to_writer(&mut *output, &doc)?;
        for doc in docs {
          writeln!(&mut *output)?;
          serde_json::to_writer(&mut *output, &doc)?;
        }
      }
      JsonStyle::Compact => {
        serde_json::to_writer(&mut *output, &doc)?;
        for doc in docs {
          write!(output, ",")?;
          serde_json::to_writer(&mut *output, &doc)?;
        }
      }
    }
    Ok(ret)
  }
}

type Buffer = Vec<u8>;

impl PrintProcessor<Buffer> for JSONProcessor {
  fn print_rule(
    &self,
    matches: Vec<NodeMatch>,
    file: SimpleFile<Cow<str>, &String>,
    rule: &RuleConfig<SgLang>,
  ) -> Result<Buffer> {
    let path = file.name();
    let jsons = matches
      .into_iter()
      .map(|nm| RuleMatchJSON::new(nm, path, rule));
    self.print_docs(jsons)
  }

  fn print_matches(&self, matches: Vec<NodeMatch>, path: &Path) -> Result<Buffer> {
    let path = path.to_string_lossy();
    let context = self.context;
    let jsons = matches
      .into_iter()
      .map(|nm| MatchJSON::new(nm, &path, context));
    self.print_docs(jsons)
  }

  fn print_diffs(&self, diffs: Vec<Diff>, path: &Path) -> Result<Buffer> {
    let path = path.to_string_lossy();
    let context = self.context;
    let jsons = diffs
      .into_iter()
      .map(|diff| MatchJSON::diff(diff, &path, context));
    self.print_docs(jsons)
  }
  fn print_rule_diffs(
    &self,
    diffs: Vec<(Diff, &RuleConfig<SgLang>)>,
    path: &Path,
  ) -> Result<Buffer> {
    let path = path.to_string_lossy();
    let jsons = diffs
      .into_iter()
      .map(|(diff, rule)| RuleMatchJSON::diff(diff, &path, rule));
    self.print_docs(jsons)
  }
}

#[cfg(test)]
mod test {
  use super::*;
  use ast_grep_config::{from_yaml_string, Fixer, GlobalRules};
  use ast_grep_language::{Language, SupportLang};

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
  fn make_test_printer(style: JsonStyle) -> JSONPrinter<Test> {
    JSONPrinter::new(Test(String::new()), style)
  }
  fn get_text(printer: &JSONPrinter<Test>) -> String {
    let output = &printer.output;
    output.0.to_string()
  }

  #[test]
  fn test_empty_printer() {
    for style in [JsonStyle::Pretty, JsonStyle::Compact] {
      let mut printer = make_test_printer(style);
      printer.before_print().unwrap();
      let buffer = printer
        .get_processor()
        .print_matches(vec![], "test.tsx".as_ref())
        .unwrap();
      printer.process(buffer).unwrap();
      printer.after_print().unwrap();
      assert_eq!(get_text(&printer), "[]\n");
    }
  }

  // source, pattern, replace, debug note
  type Case<'a> = (&'a str, &'a str, &'a str, &'a str);

  const MATCHES_CASES: &[Case] = &[
    ("let a = 123", "a", "b", "Simple match"),
    (
      "Some(1), Some(2), Some(3)",
      "Some",
      "Any",
      "Same line match",
    ),
    (
      "Some(1), Some(2)\nSome(3), Some(4)",
      "Some",
      "Any",
      "Multiple line match",
    ),
    (
      "import a from 'b';import a from 'b';",
      "import a from 'b';",
      "import c from 'b';",
      "immediate following but not overlapping",
    ),
  ];

  #[test]
  fn test_invariant() {
    for &(source, pattern, _, note) in MATCHES_CASES {
      // heading is required for CI
      let mut printer = make_test_printer(JsonStyle::Pretty);
      let grep = SgLang::from(SupportLang::Tsx).ast_grep(source);
      let matches = grep.root().find_all(pattern).collect();
      printer.before_print().unwrap();
      let buffer = printer
        .get_processor()
        .print_matches(matches, "test.tsx".as_ref())
        .unwrap();
      printer.process(buffer).unwrap();
      printer.after_print().unwrap();
      let json_str = get_text(&printer);
      let json: Vec<MatchJSON> = serde_json::from_str(&json_str).unwrap();
      assert_eq!(json[0].text, pattern, "{note}");
    }
  }

  #[test]
  fn test_replace_json() {
    for &(source, pattern, replace, note) in MATCHES_CASES {
      // heading is required for CI
      let mut printer = make_test_printer(JsonStyle::Compact);
      let lang = SgLang::from(SupportLang::Tsx);
      let grep = lang.ast_grep(source);
      let matches = grep.root().find_all(pattern);
      let fixer = Fixer::from_str(replace, &lang).expect("should work");
      let diffs = matches
        .map(|m| Diff::generate(m, &pattern, &fixer))
        .collect();
      printer.before_print().unwrap();
      let buffer = printer
        .get_processor()
        .print_diffs(diffs, "test.tsx".as_ref())
        .unwrap();
      printer.process(buffer).unwrap();
      printer.after_print().unwrap();
      let json_str = get_text(&printer);
      let json: Vec<MatchJSON> = serde_json::from_str(&json_str).unwrap();
      let actual = json[0].replacement.as_ref().expect("should have diff");
      assert_eq!(actual, replace, "{note}");
    }
  }

  fn make_rule(rule: &str) -> RuleConfig<SgLang> {
    let globals = GlobalRules::default();
    from_yaml_string(
      &format!(
        r#"
id: test
message: test rule
severity: info
language: TypeScript
note: a long random note
rule:
  pattern: "{rule}""#
      ),
      &globals,
    )
    .unwrap()
    .pop()
    .unwrap()
  }

  #[test]
  fn test_rule_json() {
    for &(source, pattern, _, note) in MATCHES_CASES {
      // TODO: understand why import does not work
      if source.contains("import") {
        continue;
      }
      let source = source.to_string();
      let mut printer = make_test_printer(JsonStyle::Pretty);
      let grep = SgLang::from(SupportLang::Tsx).ast_grep(&source);
      let rule = make_rule(pattern);
      let matches = grep.root().find_all(&rule.matcher).collect();
      printer.before_print().unwrap();
      let file = SimpleFile::new(Cow::Borrowed("test.ts"), &source);
      let buffer = printer
        .get_processor()
        .print_rule(matches, file, &rule)
        .unwrap();
      printer.process(buffer).unwrap();
      printer.after_print().unwrap();
      let json_str = get_text(&printer);
      let json: Vec<RuleMatchJSON> = serde_json::from_str(&json_str).unwrap();
      assert_eq!(json[0].matched.text, pattern, "{note}");
      assert_eq!(json[0].note, rule.note);
    }
  }

  #[test]
  fn test_single_matched_json() {
    let mut printer = make_test_printer(JsonStyle::Pretty);
    let lang = SgLang::from(SupportLang::Tsx);
    let grep = lang.ast_grep("console.log(123)");
    let matches = grep.root().find_all("console.log($A)").collect();
    printer.before_print().unwrap();
    let buffer = printer
      .get_processor()
      .print_matches(matches, "test.tsx".as_ref())
      .unwrap();
    printer.process(buffer).unwrap();
    printer.after_print().unwrap();
    let json_str = get_text(&printer);
    let json: Vec<MatchJSON> = serde_json::from_str(&json_str).unwrap();
    let actual = &json[0]
      .meta_variables
      .as_ref()
      .expect("should exist")
      .single;
    assert_eq!(actual["A"].text, "123");
  }

  #[test]
  fn test_multi_matched_json() {
    let mut printer = make_test_printer(JsonStyle::Compact);
    let lang = SgLang::from(SupportLang::Tsx);
    let grep = lang.ast_grep("console.log(1, 2, 3)");
    let matches = grep.root().find_all("console.log($$$A)").collect();
    printer.before_print().unwrap();
    let buffer = printer
      .get_processor()
      .print_matches(matches, "test.tsx".as_ref())
      .unwrap();
    printer.process(buffer).unwrap();
    printer.after_print().unwrap();
    let json_str = get_text(&printer);
    let json: Vec<MatchJSON> = serde_json::from_str(&json_str).unwrap();
    let actual = &json[0].meta_variables.as_ref().expect("should exist").multi;
    assert_eq!(actual["A"][0].text, "1");
    assert_eq!(actual["A"][2].text, "2");
    assert_eq!(actual["A"][4].text, "3");
  }

  #[test]
  fn test_streaming() {
    for &(source, pattern, _, note) in MATCHES_CASES {
      let mut printer = make_test_printer(JsonStyle::Stream);
      let grep = SgLang::from(SupportLang::Tsx).ast_grep(source);
      let matches = grep.root().find_all(pattern).collect();
      printer.before_print().unwrap();
      let buffer = printer
        .get_processor()
        .print_matches(matches, "test.tsx".as_ref())
        .unwrap();
      printer.process(buffer).unwrap();
      printer.after_print().unwrap();
      let json_str = get_text(&printer);
      let jsons: Vec<&str> = json_str.lines().collect();
      assert!(!jsons.is_empty());
      let json: Vec<MatchJSON> = jsons
        .into_iter()
        .map(|s| serde_json::from_str(s).unwrap())
        .collect();
      assert_eq!(json[0].text, pattern, "{note}");
    }
  }

  use crate::verify::test::get_rule_config;
  const TRANSFORM_TEXT: &str = "
transform:
  B:
    substring:
      source: $A
      startChar: 1
      endChar: -1
";
  #[test]
  fn test_transform() {
    let mut printer = make_test_printer(JsonStyle::Compact);
    let rule = get_rule_config(&format!("pattern: console.log($A)\n{}", TRANSFORM_TEXT));
    let grep = SgLang::from(SupportLang::TypeScript).ast_grep("console.log(123)");
    let matches = grep.root().find_all(&rule.matcher).collect();
    printer.before_print().unwrap();
    let buffer = printer
      .get_processor()
      .print_matches(matches, "test.tsx".as_ref())
      .unwrap();
    printer.process(buffer).unwrap();
    printer.after_print().unwrap();
    let json_str = get_text(&printer);
    let json: Vec<MatchJSON> = serde_json::from_str(&json_str).unwrap();
    let metas = &json[0].meta_variables.as_ref().expect("should exist");
    assert_eq!(metas.single["A"].text, "123");
    assert_eq!(metas.transformed["B"], "2");
  }
}
