//! Benchmarks for ast-grep-config CombinedScan operations.
//!
//! Run with: `cargo bench -p ast-grep-config`
//!
//! These benchmarks cover:
//! - CombinedScan construction (kind_rule_mapping, cost sorting)
//! - CombinedScan::scan() with subtree pruning + KindMask
//! - file_can_match() literal pre-filtering
//! - Single-rule vs multi-rule scan scaling

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};

use ast_grep_config::{from_str, from_yaml_string, CombinedScan, GlobalRules, RuleConfig};
use ast_grep_core::matcher::{Pattern, PatternBuilder, PatternError};
use ast_grep_core::tree_sitter::{LanguageExt, StrDoc, TSLanguage};
use ast_grep_core::Language;
use serde::Deserialize;
use std::path::Path;

// -- Language definition (mirrors the #[cfg(test)] TypeScript in config/lib.rs) --

#[derive(Clone, Deserialize, PartialEq, Eq)]
enum TypeScript {
  Tsx,
}

impl Language for TypeScript {
  fn kind_to_id(&self, kind: &str) -> u16 {
    TSLanguage::from(tree_sitter_typescript::LANGUAGE_TSX).id_for_node_kind(kind, true)
  }
  fn field_to_id(&self, field: &str) -> Option<u16> {
    TSLanguage::from(tree_sitter_typescript::LANGUAGE_TSX)
      .field_id_for_name(field)
      .map(|f| f.get())
  }
  fn from_path<P: AsRef<Path>>(_path: P) -> Option<Self> {
    Some(TypeScript::Tsx)
  }
  fn build_pattern(&self, builder: &PatternBuilder) -> Result<Pattern, PatternError> {
    builder.build(|src| StrDoc::try_new(src, self.clone()))
  }
}

impl LanguageExt for TypeScript {
  fn get_ts_language(&self) -> TSLanguage {
    tree_sitter_typescript::LANGUAGE_TSX.into()
  }
}

// -- Helpers --

fn make_rule(id: &str, pattern: &str) -> String {
  format!(
    r#"
id: {id}
message: bench rule
severity: info
language: Tsx
rule:
  pattern: '{pattern}'
"#
  )
}

fn parse_rules(yamls: &[String]) -> Vec<RuleConfig<TypeScript>> {
  let globals = GlobalRules::default();
  let combined = yamls.join("\n---\n");
  from_yaml_string::<TypeScript>(&combined, &globals).expect("rules should parse")
}

fn wide_code(count: usize) -> String {
  let mut s = String::new();
  for i in 0..count {
    s.push_str(&format!("const v{i} = console.log({i});\n"));
  }
  s
}

fn mixed_code(count: usize) -> String {
  let mut s = String::new();
  for i in 0..count {
    match i % 4 {
      0 => s.push_str(&format!("console.log({i});\n")),
      1 => s.push_str(&format!("console.warn({i});\n")),
      2 => s.push_str(&format!("console.error({i});\n")),
      _ => s.push_str(&format!("const x{i} = {i};\n")),
    }
  }
  s
}

// ─── Benchmark groups ───────────────────────────────────────────────

fn bench_combined_scan_construction(c: &mut Criterion) {
  let mut group = c.benchmark_group("combined_scan_construction");

  for &rule_count in &[1, 5, 20] {
    let yamls: Vec<String> = (0..rule_count)
      .map(|i| make_rule(&format!("rule{i}"), "console.log($A)"))
      .collect();
    let rules = parse_rules(&yamls);
    let rule_refs: Vec<&RuleConfig<TypeScript>> = rules.iter().collect();

    group.bench_with_input(
      BenchmarkId::new("rules", rule_count),
      &rule_refs,
      |b, refs| {
        b.iter(|| {
          let scan = CombinedScan::new(black_box(refs.clone()));
          black_box(scan);
        });
      },
    );
  }

  group.finish();
}

fn bench_scan_single_rule(c: &mut Criterion) {
  let mut group = c.benchmark_group("scan_single_rule");

  let yamls = vec![make_rule("log", "console.log($A)")];
  let rules = parse_rules(&yamls);

  for &size in &[50, 200, 1000] {
    let code = wide_code(size);
    let root = TypeScript::Tsx.ast_grep(&code);
    let rule_refs: Vec<&RuleConfig<TypeScript>> = rules.iter().collect();
    let scan = CombinedScan::new(rule_refs);

    group.bench_with_input(BenchmarkId::new("statements", size), &root, |b, root| {
      b.iter(|| {
        let result = scan.scan(black_box(root), false);
        let total: usize = result.matches.iter().map(|(_, m)| m.len()).sum();
        assert_eq!(total, size);
      });
    });
  }

  group.finish();
}

fn bench_scan_multi_rule(c: &mut Criterion) {
  let mut group = c.benchmark_group("scan_multi_rule");

  let yamls = vec![
    make_rule("log", "console.log($A)"),
    make_rule("warn", "console.warn($A)"),
    make_rule("error", "console.error($A)"),
  ];
  let rules = parse_rules(&yamls);

  for &size in &[100, 500] {
    let code = mixed_code(size);
    let root = TypeScript::Tsx.ast_grep(&code);
    let rule_refs: Vec<&RuleConfig<TypeScript>> = rules.iter().collect();
    let scan = CombinedScan::new(rule_refs);

    group.bench_with_input(BenchmarkId::new("statements", size), &root, |b, root| {
      b.iter(|| {
        let result = scan.scan(black_box(root), false);
        let total: usize = result.matches.iter().map(|(_, m)| m.len()).sum();
        black_box(total);
      });
    });
  }

  group.finish();
}

fn bench_file_can_match(c: &mut Criterion) {
  let mut group = c.benchmark_group("file_can_match");

  let yamls = vec![
    make_rule("log", "console.log($A)"),
    make_rule("warn", "console.warn($A)"),
  ];
  let rules = parse_rules(&yamls);
  let rule_refs: Vec<&RuleConfig<TypeScript>> = rules.iter().collect();
  let scan = CombinedScan::new(rule_refs);

  // File that matches
  let matching = b"const x = 1;\nconsole.log('hello');\n";
  group.bench_function("matching_file", |b| {
    b.iter(|| {
      assert!(scan.file_can_match(black_box(matching)));
    });
  });

  // File that doesn't match any rule
  let non_matching = b"const x = 1;\nconst y = 2;\nfunction foo() { return x + y; }\n";
  group.bench_function("non_matching_file", |b| {
    b.iter(|| {
      let result = scan.file_can_match(black_box(non_matching));
      black_box(result);
    });
  });

  // Large file scan
  let large_non_matching: Vec<u8> = "const x = 1;\n".repeat(10000).into_bytes();
  group.bench_function("large_non_matching", |b| {
    b.iter(|| {
      let result = scan.file_can_match(black_box(&large_non_matching));
      black_box(result);
    });
  });

  group.finish();
}

fn bench_subtree_pruning(c: &mut Criterion) {
  let mut group = c.benchmark_group("subtree_pruning");

  let yamls = vec![make_rule("log", "console.log($A)")];
  let rules = parse_rules(&yamls);

  // Deeply nested code where the match is at the bottom
  let mut deep_code = String::new();
  for i in 0..100 {
    deep_code.push_str(&format!("function f{i}() {{\n"));
  }
  deep_code.push_str("console.log('found');\n");
  for _ in 0..100 {
    deep_code.push_str("}\n");
  }
  let root = TypeScript::Tsx.ast_grep(&deep_code);
  let rule_refs: Vec<&RuleConfig<TypeScript>> = rules.iter().collect();
  let scan = CombinedScan::new(rule_refs);

  group.bench_function("deep_nest_100", |b| {
    b.iter(|| {
      let result = scan.scan(black_box(&root), false);
      let total: usize = result.matches.iter().map(|(_, m)| m.len()).sum();
      assert_eq!(total, 1);
    });
  });

  group.finish();
}

criterion_group!(
  benches,
  bench_combined_scan_construction,
  bench_scan_single_rule,
  bench_scan_multi_rule,
  bench_file_can_match,
  bench_subtree_pruning,
);
criterion_main!(benches);
