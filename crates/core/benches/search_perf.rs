//! Benchmarks for ast-grep core search operations.
//!
//! Run with: `cargo bench -p ast-grep-core`
//!
//! These benchmarks cover the hot paths optimised in the implementation plan:
//! - Pattern matching (single pattern search)
//! - find_all traversal with kind filtering
//! - MetaVarEnv operations (SmallVec-backed)
//! - KindMask vs BitSet membership checks
//! - Pattern fingerprint rejection
//! - Structural hash in does_node_match_exactly

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};

use ast_grep_core::kind_mask::KindMask;
use ast_grep_core::matcher::{MatcherExt, Pattern};
use ast_grep_core::tree_sitter::{LanguageExt, StrDoc, TSLanguage};
use ast_grep_core::{AstGrep, Language, PatternError};

use bit_set::BitSet;

// -- Language definition (mirrors the #[cfg(test)] Tsx in language.rs) --

#[derive(Clone)]
struct Tsx;

impl Language for Tsx {
  fn kind_to_id(&self, kind: &str) -> u16 {
    let ts_lang: TSLanguage = tree_sitter_typescript::LANGUAGE_TSX.into();
    ts_lang.id_for_node_kind(kind, true)
  }
  fn field_to_id(&self, field: &str) -> Option<u16> {
    self
      .get_ts_language()
      .field_id_for_name(field)
      .map(|f| f.get())
  }
  fn build_pattern(
    &self,
    builder: &ast_grep_core::matcher::PatternBuilder,
  ) -> Result<Pattern, PatternError> {
    builder.build(|src| StrDoc::try_new(src, self.clone()))
  }
}

impl LanguageExt for Tsx {
  fn get_ts_language(&self) -> TSLanguage {
    tree_sitter_typescript::LANGUAGE_TSX.into()
  }
}

// -- Helpers --

fn parse(src: &str) -> AstGrep<StrDoc<Tsx>> {
  Tsx.ast_grep(src)
}

/// Generate a deeply nested function body for stress testing.
fn nested_code(depth: usize) -> String {
  let mut s = String::new();
  for i in 0..depth {
    s.push_str(&format!("function f{}() {{\n", i));
  }
  s.push_str("const target = 42;\n");
  for _ in 0..depth {
    s.push_str("}\n");
  }
  s
}

/// Generate a wide file with many statements.
fn wide_code(count: usize) -> String {
  let mut s = String::new();
  for i in 0..count {
    s.push_str(&format!("const v{} = console.log({});\n", i, i));
  }
  s
}

// ─── Benchmark groups ───────────────────────────────────────────────

fn bench_pattern_match(c: &mut Criterion) {
  let mut group = c.benchmark_group("pattern_match");

  // Simple leaf pattern
  let code = "const a = 123; const b = 456; const c = 789;";
  let root = parse(code);
  let pattern = Pattern::new("const $A = $B", Tsx);
  group.bench_function("simple_const", |b| {
    b.iter(|| {
      let count = root.root().find_all(black_box(&pattern)).count();
      assert_eq!(count, 3);
    });
  });

  // Function call pattern
  let code2 = "console.log(1); console.log(2); console.warn(3); console.log(4);";
  let root2 = parse(code2);
  let pattern2 = Pattern::new("console.log($A)", Tsx);
  group.bench_function("console_log", |b| {
    b.iter(|| {
      let count = root2.root().find_all(black_box(&pattern2)).count();
      assert_eq!(count, 3);
    });
  });

  // Ellipsis pattern
  let code3 = "foo(1, 2, 3); foo(a); foo(x, y);";
  let root3 = parse(code3);
  let pattern3 = Pattern::new("foo($$$)", Tsx);
  group.bench_function("ellipsis", |b| {
    b.iter(|| {
      let count = root3.root().find_all(black_box(&pattern3)).count();
      assert_eq!(count, 3);
    });
  });

  group.finish();
}

fn bench_find_all_scaling(c: &mut Criterion) {
  let mut group = c.benchmark_group("find_all_scaling");
  let pattern = Pattern::new("console.log($A)", Tsx);

  for &size in &[50, 200, 1000] {
    let code = wide_code(size);
    let root = parse(&code);
    group.bench_with_input(BenchmarkId::new("wide", size), &root, |b, root| {
      b.iter(|| {
        let count = root.root().find_all(black_box(&pattern)).count();
        assert_eq!(count, size);
      });
    });
  }

  group.finish();
}

fn bench_deep_nesting(c: &mut Criterion) {
  let mut group = c.benchmark_group("deep_nesting");
  let pattern = Pattern::new("const target = 42", Tsx);

  for &depth in &[10, 50, 200] {
    let code = nested_code(depth);
    let root = parse(&code);
    group.bench_with_input(BenchmarkId::new("depth", depth), &root, |b, root| {
      b.iter(|| {
        let found = root.root().find(black_box(&pattern));
        assert!(found.is_some());
      });
    });
  }

  group.finish();
}

fn bench_meta_var_env(c: &mut Criterion) {
  let mut group = c.benchmark_group("meta_var_env");

  // Benchmark pattern matching that exercises MetaVarEnv insert/lookup
  let code = "function test(a, b, c) { return a + b + c; }";
  let root = parse(code);

  let pattern = Pattern::new("function $FN($$$ARGS) { return $BODY; }", Tsx);
  group.bench_function("capture_vars", |b| {
    b.iter(|| {
      let found = root.root().find(black_box(&pattern));
      assert!(found.is_some());
      let m = found.unwrap();
      black_box(m.get_env().get_match("FN"));
    });
  });

  // Multiple matches with env creation per match
  let code2 = wide_code(100);
  let root2 = parse(&code2);
  let pattern2 = Pattern::new("console.log($A)", Tsx);
  group.bench_function("many_matches", |b| {
    b.iter(|| {
      let count = root2.root().find_all(black_box(&pattern2)).count();
      assert_eq!(count, 100);
    });
  });

  group.finish();
}

fn bench_kind_mask(c: &mut Criterion) {
  let mut group = c.benchmark_group("kind_mask");

  // Build a realistic BitSet (simulate ~10 rule kinds)
  let mut bitset = BitSet::new();
  for kind in [15, 42, 78, 120, 200, 255, 300, 350, 400, 450] {
    bitset.insert(kind);
  }
  let kind_mask = KindMask::from_bitset(&bitset);

  // Membership checks — the hot path in CombinedScan::scan()
  let test_kinds: Vec<usize> = (0..500).collect();

  group.bench_function("bitset_contains_500", |b| {
    b.iter(|| {
      let mut hits = 0usize;
      for &k in &test_kinds {
        if bitset.contains(k) {
          hits += 1;
        }
      }
      assert_eq!(hits, 10);
    });
  });

  group.bench_function("kindmask_contains_500", |b| {
    b.iter(|| {
      let mut hits = 0usize;
      for &k in &test_kinds {
        if kind_mask.contains(k) {
          hits += 1;
        }
      }
      assert_eq!(hits, 10);
    });
  });

  // Union performance
  let mut bitset2 = BitSet::new();
  for kind in [10, 50, 100, 150, 250, 350, 450] {
    bitset2.insert(kind);
  }
  let kind_mask2 = KindMask::from_bitset(&bitset2);

  group.bench_function("bitset_union", |b| {
    b.iter(|| {
      let mut bs = bitset.clone();
      bs.union_with(black_box(&bitset2));
      black_box(bs);
    });
  });

  group.bench_function("kindmask_union", |b| {
    b.iter(|| {
      let mut km = kind_mask.clone();
      km.union_with(black_box(&kind_mask2));
      black_box(km);
    });
  });

  group.finish();
}

fn bench_pattern_fingerprint(c: &mut Criterion) {
  let mut group = c.benchmark_group("fingerprint_rejection");

  // Pattern with fingerprint
  let pattern = Pattern::new("function $A($$$) { $$$ }", Tsx);
  let code = wide_code(200);
  let root = parse(&code);

  // Measure how many nodes are rejected by fingerprint vs full match
  group.bench_function("find_with_fingerprint", |b| {
    b.iter(|| {
      let count = root.root().find_all(black_box(&pattern)).count();
      assert_eq!(count, 0);
    });
  });

  group.finish();
}

fn bench_str_pattern_cache(c: &mut Criterion) {
  let mut group = c.benchmark_group("pattern_cache");

  let code = "const a = 1; const b = 2; const c = 3;";
  let root = parse(code);

  // Using &str as Matcher (exercises thread-local LRU cache)
  group.bench_function("str_matcher_cached", |b| {
    b.iter(|| {
      let found = root.root().find(black_box("const $A = $B"));
      assert!(found.is_some());
    });
  });

  // Using pre-compiled Pattern (no cache lookup)
  let pattern = Pattern::new("const $A = $B", Tsx);
  group.bench_function("compiled_pattern", |b| {
    b.iter(|| {
      let found = root.root().find(black_box(&pattern));
      assert!(found.is_some());
    });
  });

  group.finish();
}

criterion_group!(
  benches,
  bench_pattern_match,
  bench_find_all_scaling,
  bench_deep_nesting,
  bench_meta_var_env,
  bench_kind_mask,
  bench_pattern_fingerprint,
  bench_str_pattern_cache,
);
criterion_main!(benches);
