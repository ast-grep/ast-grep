use ast_grep_config::{from_yaml_string, RuleConfig};
use ast_grep_core::{AstGrep, Language, Matcher, Pattern, StrDoc};
use ast_grep_language::SupportLang;
use criterion::{black_box, criterion_group, criterion_main, Criterion};
use std::env::current_dir;
use std::fs::read_to_string;

fn read_rule() -> RuleConfig<SupportLang> {
  let cwd = current_dir().unwrap();
  let ts_file = cwd.join("fixtures/rules/has-rule.yml");
  let rule = read_to_string(ts_file).unwrap();
  let mut rules = from_yaml_string(&rule, &Default::default()).unwrap();
  rules.pop().unwrap()
}

fn find_pattern<M: Matcher>(sg: &AstGrep<StrDoc<SupportLang>>, pattern: &M) {
  sg.root().find_all(pattern).for_each(drop);
}

fn get_sg(path: &str) -> AstGrep<StrDoc<SupportLang>> {
  let lang = SupportLang::TypeScript;
  let cwd = current_dir().unwrap();
  let ts_file = cwd.join(path);
  let checker_source = read_to_string(ts_file).unwrap();
  lang.ast_grep(checker_source)
}

fn find_all_bench(c: &mut Criterion) {
  let lang = SupportLang::TypeScript;
  let pattern = Pattern::new(black_box("$A && $A()"), lang);
  let checker_sg = get_sg("fixtures/checker.ts.fixture");
  let tsc_sg = get_sg("fixtures/tsc.ts.fixture");
  let ref_sg = get_sg("fixtures/ref.ts.fixture");
  c.bench_function("large file(checker.ts)", |b| {
    b.iter(|| find_pattern(&checker_sg, &pattern))
  });
  c.bench_function("medium file(ref.ts)", |b| {
    b.iter(|| find_pattern(&ref_sg, &pattern))
  });
  c.bench_function("small file(tsc.ts)", |b| {
    b.iter(|| find_pattern(&tsc_sg, &pattern))
  });
}

fn rule_bench(c: &mut Criterion) {
  let ref_sg = get_sg("fixtures/ref.ts.fixture");
  let rule = read_rule();
  c.bench_function("test has rule", |b| {
    b.iter(|| find_pattern(&ref_sg, &rule.matcher))
  });
}

fn build_pattern_bench(c: &mut Criterion) {
  let lang = SupportLang::TypeScript;
  c.bench_function("Build Normal Pattern", |b| {
    b.iter(|| Pattern::new(black_box("function $FUNC($$$ARGS) { $$$BODY }"), lang))
  });
}

criterion_group!(benches, find_all_bench, rule_bench, build_pattern_bench);
criterion_main!(benches);
