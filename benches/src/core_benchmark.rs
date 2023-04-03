use ast_grep_core::{AstGrep, Language, Pattern};
use ast_grep_language::SupportLang;
use criterion::{black_box, criterion_group, criterion_main, Criterion};
use std::env::current_dir;
use std::fs::read_to_string;

fn find_pattern(sg: &AstGrep<SupportLang>, pattern: &Pattern<SupportLang>) {
  sg.root().find_all(pattern).for_each(|n| drop(n));
}

fn get_sg(path: &str) -> AstGrep<SupportLang> {
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

criterion_group!(benches, find_all_bench);
criterion_main!(benches);
