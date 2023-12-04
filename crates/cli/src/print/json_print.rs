use crate::lang::SgLang;
use ast_grep_config::{RuleConfig, Severity};
use ast_grep_core::{meta_var::MetaVariable, Node as SgNode, NodeMatch as SgNodeMatch, StrDoc};

type NodeMatch<'a, L> = SgNodeMatch<'a, StrDoc<L>>;
type Node<'a, L> = SgNode<'a, StrDoc<L>>;

use std::collections::HashMap;

use super::{Diff, Printer};
use anyhow::Result;
use clap::ValueEnum;
use codespan_reporting::files::SimpleFile;
use serde::{Deserialize, Serialize};

use std::borrow::Cow;
use std::io::{Stdout, Write};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;

// add this macro because neither trait_alias nor type_alias_impl is supported.
macro_rules! Matches {
  ($lt: lifetime) => { impl Iterator<Item = NodeMatch<$lt, SgLang>> };
}
macro_rules! Diffs {
  ($lt: lifetime) => { impl Iterator<Item = Diff<$lt>> };
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Position {
  line: usize,
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

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct MatchJSON<'a> {
  text: Cow<'a, str>,
  range: Range,
  file: Cow<'a, str>,
  lines: String,
  #[serde(skip_serializing_if = "Option::is_none")]
  replacement: Option<Cow<'a, str>>,
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
fn from_env<'a>(nm: &NodeMatch<'a, SgLang>) -> Option<MetaVariables<'a>> {
  let env = nm.get_env();
  let mut vars = env.get_matched_variables().peekable();
  vars.peek()?;
  let mut single = HashMap::new();
  let mut multi = HashMap::new();
  let mut transformed = HashMap::new();
  for var in vars {
    use MetaVariable as MV;
    match var {
      MV::Named(n, _) => {
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
      MV::NamedEllipsis(n) => {
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
      line: start_pos.0,
      column: start_pos.1,
    },
    end: Position {
      line: end_pos.0,
      column: end_pos.1,
    },
  }
}

impl<'a> MatchJSON<'a> {
  fn new(nm: NodeMatch<'a, SgLang>, path: &'a str) -> Self {
    let display = nm.display_context(0, 0);
    let lines = format!("{}{}{}", display.leading, display.matched, display.trailing);
    MatchJSON {
      file: Cow::Borrowed(path),
      text: nm.text(),
      lines,
      language: *nm.lang(),
      replacement: None,
      range: get_range(&nm),
      meta_variables: from_env(&nm),
    }
  }
}
fn get_labels<'a>(nm: &NodeMatch<'a, SgLang>) -> Option<Vec<MatchNode<'a>>> {
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
  fn new(nm: NodeMatch<'a, SgLang>, path: &'a str, rule: &'a RuleConfig<SgLang>) -> Self {
    let message = rule.get_message(&nm);
    let labels = get_labels(&nm);
    let matched = MatchJSON::new(nm, path);
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
  output: Mutex<W>,
  style: JsonStyle,
  // indicate if any matches happened
  matched: AtomicBool,
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
      output: Mutex::new(output),
      matched: AtomicBool::new(false),
    }
  }

  fn print_docs<S: Serialize>(&self, mut docs: impl Iterator<Item = S>) -> Result<()> {
    let Some(doc) = docs.next() else {
      return Ok(());
    };
    let mut lock = self.output.lock().expect("should work");
    let matched = self.matched.swap(true, Ordering::AcqRel);
    match self.style {
      JsonStyle::Pretty => {
        if matched {
          writeln!(&mut lock, ",")?;
        } else {
          writeln!(&mut lock)?;
        }
        serde_json::to_writer_pretty(&mut *lock, &doc)?;
        for doc in docs {
          writeln!(&mut lock, ",")?;
          serde_json::to_writer_pretty(&mut *lock, &doc)?;
        }
      }
      JsonStyle::Stream => {
        serde_json::to_writer(&mut *lock, &doc)?;
        writeln!(&mut lock)?;
        for doc in docs {
          serde_json::to_writer(&mut *lock, &doc)?;
          writeln!(&mut lock)?;
        }
      }
      JsonStyle::Compact => {
        if matched {
          write!(&mut lock, ",")?;
        }
        serde_json::to_writer(&mut *lock, &doc)?;
        for doc in docs {
          write!(&mut lock, ",")?;
          serde_json::to_writer(&mut *lock, &doc)?;
        }
      }
    }
    Ok(())
  }
}

impl<W: Write> Printer for JSONPrinter<W> {
  fn print_rule<'a>(
    &self,
    matches: Matches!('a),
    file: SimpleFile<Cow<str>, &String>,
    rule: &RuleConfig<SgLang>,
  ) -> Result<()> {
    let path = file.name();
    let jsons = matches.map(|nm| RuleMatchJSON::new(nm, path, rule));
    self.print_docs(jsons)
  }

  fn print_matches<'a>(&self, matches: Matches!('a), path: &Path) -> Result<()> {
    let path = path.to_string_lossy();
    let jsons = matches.map(|nm| MatchJSON::new(nm, &path));
    self.print_docs(jsons)
  }

  fn print_diffs<'a>(&self, diffs: Diffs!('a), path: &Path) -> Result<()> {
    let path = path.to_string_lossy();
    let jsons = diffs.map(|diff| {
      let mut v = MatchJSON::new(diff.node_match, &path);
      v.replacement = Some(diff.replacement);
      v
    });
    self.print_docs(jsons)
  }
  fn print_rule_diffs(
    &self,
    diffs: Vec<(Diff<'_>, &RuleConfig<SgLang>)>,
    path: &Path,
  ) -> Result<()> {
    let path = path.to_string_lossy();
    let jsons = diffs.into_iter().map(|(diff, rule)| {
      let mut v = RuleMatchJSON::new(diff.node_match, &path, rule);
      v.matched.replacement = Some(diff.replacement);
      v
    });
    self.print_docs(jsons)
  }

  fn before_print(&self) -> Result<()> {
    if self.style == JsonStyle::Stream {
      return Ok(());
    }
    let mut lock = self.output.lock().expect("should work");
    write!(&mut lock, "[")?;
    Ok(())
  }

  fn after_print(&self) -> Result<()> {
    if self.style == JsonStyle::Stream {
      return Ok(());
    }
    let mut lock = self.output.lock().expect("should work");
    let matched = self.matched.load(Ordering::Acquire);
    if matched && self.style == JsonStyle::Pretty {
      writeln!(&mut lock)?;
    }
    writeln!(&mut lock, "]")?;
    Ok(())
  }
}

#[cfg(test)]
mod test {
  use super::*;
  use ast_grep_config::{from_yaml_string, GlobalRules};
  use ast_grep_core::replacer::TemplateFix;
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
    let lock = printer.output.lock().unwrap();
    lock.0.to_string()
  }

  #[test]
  fn test_empty_printer() {
    for style in [JsonStyle::Pretty, JsonStyle::Compact] {
      let printer = make_test_printer(style);
      printer.before_print().unwrap();
      printer
        .print_matches(std::iter::empty(), "test.tsx".as_ref())
        .unwrap();
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
      let printer = make_test_printer(JsonStyle::Pretty);
      let grep = SgLang::from(SupportLang::Tsx).ast_grep(source);
      let matches = grep.root().find_all(pattern);
      printer.before_print().unwrap();
      printer.print_matches(matches, "test.tsx".as_ref()).unwrap();
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
      let printer = make_test_printer(JsonStyle::Compact);
      let lang = SgLang::from(SupportLang::Tsx);
      let grep = lang.ast_grep(source);
      let matches = grep.root().find_all(pattern);
      let fixer = TemplateFix::try_new(replace, &lang).expect("should work");
      let diffs = matches.map(|m| Diff::generate(m, &pattern, &fixer));
      printer.before_print().unwrap();
      printer.print_diffs(diffs, "test.tsx".as_ref()).unwrap();
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
      let printer = make_test_printer(JsonStyle::Pretty);
      let grep = SgLang::from(SupportLang::Tsx).ast_grep(&source);
      let rule = make_rule(pattern);
      let matches = grep.root().find_all(&rule.matcher);
      printer.before_print().unwrap();
      let file = SimpleFile::new(Cow::Borrowed("test.ts"), &source);
      printer.print_rule(matches, file, &rule).unwrap();
      printer.after_print().unwrap();
      let json_str = get_text(&printer);
      let json: Vec<RuleMatchJSON> = serde_json::from_str(&json_str).unwrap();
      assert_eq!(json[0].matched.text, pattern, "{note}");
      assert_eq!(json[0].note, rule.note);
    }
  }

  #[test]
  fn test_single_matched_json() {
    let printer = make_test_printer(JsonStyle::Pretty);
    let lang = SgLang::from(SupportLang::Tsx);
    let grep = lang.ast_grep("console.log(123)");
    let matches = grep.root().find_all("console.log($A)");
    printer.before_print().unwrap();
    printer.print_matches(matches, "test.tsx".as_ref()).unwrap();
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
    let printer = make_test_printer(JsonStyle::Compact);
    let lang = SgLang::from(SupportLang::Tsx);
    let grep = lang.ast_grep("console.log(1, 2, 3)");
    let matches = grep.root().find_all("console.log($$$A)");
    printer.before_print().unwrap();
    printer.print_matches(matches, "test.tsx".as_ref()).unwrap();
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
      let printer = make_test_printer(JsonStyle::Stream);
      let grep = SgLang::from(SupportLang::Tsx).ast_grep(source);
      let matches = grep.root().find_all(pattern);
      printer.before_print().unwrap();
      printer.print_matches(matches, "test.tsx".as_ref()).unwrap();
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
    let printer = make_test_printer(JsonStyle::Compact);
    let rule = get_rule_config(&format!("pattern: console.log($A)\n{}", TRANSFORM_TEXT));
    let grep = SgLang::from(SupportLang::TypeScript).ast_grep("console.log(123)");
    let matches = grep.root().find_all(&rule.matcher);
    printer.before_print().unwrap();
    printer.print_matches(matches, "test.tsx".as_ref()).unwrap();
    printer.after_print().unwrap();
    let json_str = get_text(&printer);
    let json: Vec<MatchJSON> = serde_json::from_str(&json_str).unwrap();
    let metas = &json[0].meta_variables.as_ref().expect("should exist");
    assert_eq!(metas.single["A"].text, "123");
    assert_eq!(metas.transformed["B"], "2");
  }
}
