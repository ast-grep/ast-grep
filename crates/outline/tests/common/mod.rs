use ast_grep_language::{LanguageExt, SupportLang};
use ast_grep_outline::{
  combined_extractor::CombinedExtractors,
  extractor::parse_outline_rules,
  model::{OutlineItem, OutlineMember},
};

fn compile_rules_for(lang: SupportLang, src: &str) -> CombinedExtractors<SupportLang> {
  let rules = parse_outline_rules::<SupportLang>(src)
    .expect("outline rules should deserialize")
    .into_iter()
    .filter(|rule| rule.common().language == lang)
    .collect::<Vec<_>>();
  CombinedExtractors::try_from(rules, &Default::default()).expect("outline rules should compile")
}

pub fn assert_outline_snapshot(lang: SupportLang, rules: &str, source: &str, expected: &str) {
  let combined = compile_rules_for(lang, rules);
  let grep = lang.ast_grep(source);
  let items = combined.extract(grep.root()).collect::<Vec<_>>();
  let snapshot = outline_snapshot(&items);
  assert_eq!(snapshot.trim(), expected.trim());
}

pub fn assert_outline_signature_snapshot(
  lang: SupportLang,
  rules: &str,
  source: &str,
  expected: &str,
) {
  let combined = compile_rules_for(lang, rules);
  let grep = lang.ast_grep(source);
  let items = combined.extract(grep.root()).collect::<Vec<_>>();
  let snapshot = outline_signature_snapshot(&items);
  assert_eq!(snapshot.trim(), expected.trim());
}

fn outline_snapshot(items: &[OutlineItem<'_>]) -> String {
  let mut output = String::new();
  for item in items {
    push_item(&mut output, item);
  }
  output
}

fn outline_signature_snapshot(items: &[OutlineItem<'_>]) -> String {
  let mut output = String::new();
  for item in items {
    push_item_signature(&mut output, item);
  }
  output
}

fn push_item(output: &mut String, item: &OutlineItem<'_>) {
  let role = if item.is_import { "import" } else { "item" };
  let visibility = if item.is_exported {
    "exported"
  } else {
    "private"
  };
  output.push_str(&format!(
    "- {:?} {role} {visibility} {}\n",
    item.entry.symbol_type, item.entry.name
  ));
  for member in &item.members {
    push_member(output, member);
  }
}

fn push_item_signature(output: &mut String, item: &OutlineItem<'_>) {
  let role = if item.is_import { "import" } else { "item" };
  let visibility = if item.is_exported {
    "exported"
  } else {
    "private"
  };
  output.push_str(&format!(
    "- {:?} {role} {visibility} {} | {}\n",
    item.entry.symbol_type, item.entry.name, item.entry.signature
  ));
  for member in &item.members {
    push_member_signature(output, member);
  }
}

fn push_member(output: &mut String, member: &OutlineMember<'_>) {
  push_member_at(output, member, 1, false);
}

fn push_member_signature(output: &mut String, member: &OutlineMember<'_>) {
  push_member_at(output, member, 1, true);
}

fn push_member_at(
  output: &mut String,
  member: &OutlineMember<'_>,
  depth: usize,
  with_signature: bool,
) {
  let visibility = if member.is_public {
    "public"
  } else {
    "private"
  };
  let indent = "  ".repeat(depth);
  let signature = if with_signature {
    format!(" | {}", member.entry.signature)
  } else {
    String::new()
  };
  output.push_str(&format!(
    "{indent}- {:?} {visibility} {}{signature}\n",
    member.entry.symbol_type, member.entry.name
  ));
  for child in &member.members {
    push_member_at(output, child, depth + 1, with_signature);
  }
}
