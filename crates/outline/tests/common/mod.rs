#![allow(dead_code)]

use ast_grep_language::{LanguageExt, SupportLang};
use ast_grep_outline::{
  combined_extractor::CombinedExtractors,
  extractor::{SerializableOutlineRule, parse_outline_rules},
  model::{OutlineItem, OutlineMember},
};

pub fn compile_rules(src: &str) -> CombinedExtractors<SupportLang> {
  let rules = parse_outline_rules::<SupportLang>(src).expect("outline rules should deserialize");
  CombinedExtractors::try_from(rules, &Default::default()).expect("outline rules should compile")
}

pub fn compile_rules_for(lang: SupportLang, src: &str) -> CombinedExtractors<SupportLang> {
  let rules = parse_outline_rules::<SupportLang>(src)
    .expect("outline rules should deserialize")
    .into_iter()
    .filter(|rule| rule.common().language == lang)
    .collect::<Vec<_>>();
  CombinedExtractors::try_from(rules, &Default::default()).expect("outline rules should compile")
}

pub fn assert_rules_compile(src: &'static str) {
  let rules = parse_outline_rules::<SupportLang>(src).expect("outline YAML should parse");
  for rule in rules {
    match rule {
      SerializableOutlineRule::Item(item) => {
        ast_grep_outline::extractor::ItemExtractor::try_from(item, &Default::default())
          .expect("item rule should compile");
      }
      SerializableOutlineRule::Member(member) => {
        ast_grep_outline::extractor::MemberExtractor::try_from(member, &Default::default())
          .expect("member rule should compile");
      }
    }
  }
}

pub fn assert_outline_snapshot(
  lang: SupportLang,
  combined: &CombinedExtractors<SupportLang>,
  source: &str,
  expected: &str,
) {
  let grep = lang.ast_grep(source);
  let snapshot = outline_snapshot(&combined.extract(grep.root()));
  assert_eq!(snapshot.trim(), expected.trim());
}

fn outline_snapshot(items: &[OutlineItem<'_>]) -> String {
  let mut output = String::new();
  for item in items {
    push_item(&mut output, item);
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

fn push_member(output: &mut String, member: &OutlineMember<'_>) {
  let visibility = if member.is_public {
    "public"
  } else {
    "private"
  };
  output.push_str(&format!(
    "  - {:?} {visibility} {}\n",
    member.entry.symbol_type, member.entry.name
  ));
}
