//! Bundled outline extractor rules.
//!
//! Built-ins use the same YAML schema as user-provided `--outline-rules`
//! files. Keeping them data-driven preserves the same loading and execution path
//! for built-in and custom languages.

pub const DEFAULT_OUTLINE_RULES: &str = concat!(
  include_str!("default_rules/rust.yml"),
  "\n---\n",
  include_str!("default_rules/typescript.yml"),
  "\n---\n",
  include_str!("default_rules/javascript.yml"),
  "\n---\n",
  include_str!("default_rules/python.yml"),
  "\n---\n",
  include_str!("default_rules/go.yml"),
  "\n---\n",
  include_str!("default_rules/kotlin.yml"),
  "\n---\n",
  include_str!("default_rules/java.yml"),
  "\n---\n",
  include_str!("default_rules/swift.yml"),
);

#[cfg(test)]
mod tests {
  use super::DEFAULT_OUTLINE_RULES;
  use crate::{
    combined_extractor::CombinedExtractors, extractor::parse_outline_rules, model::SymbolType,
  };
  use ast_grep_core::tree_sitter::LanguageExt;
  use ast_grep_language::SupportLang;

  fn rust_combined() -> CombinedExtractors<SupportLang> {
    let rules = parse_outline_rules::<SupportLang>(DEFAULT_OUTLINE_RULES)
      .expect("builtin outline rules should deserialize")
      .into_iter()
      .filter(|rule| rule.common().language == SupportLang::Rust)
      .collect::<Vec<_>>();
    CombinedExtractors::try_from(rules, &Default::default()).expect("rules should compile")
  }

  #[test]
  fn rust_builtin_rules_extract_file_outline() {
    let combined = rust_combined();
    let grep = SupportLang::Rust.ast_grep(
      r#"
pub use crate::api::Parser;
use std::fmt;

pub struct Config {
  pub name: String,
  enabled: bool,
}

enum Mode {
  Fast,
  Slow,
  RuleConfig(#[from] RuleConfigError),
  Predicate(#[from] RuleSerializeError),
  Template(#[from] TemplateFixError),
  Complex {
    /// nth-child syntax
    position: NthChildSimple,
    /// select the nth node that matches the rule, like CSS's of syntax
    of_rule: Option<Box<SerializableRule>>,
    /// matches from the end instead like CSS's nth-last-child
    #[serde(default)]
    reverse: bool,
  },
}

impl Config {
  pub fn new(name: String) -> Self {
    Self { name, enabled: true }
  }

  fn enabled(&self) -> bool {
    self.enabled
  }
}

fn helper() {}

mod tests {
  fn nested_helper() {}

  #[test]
  fn parses_config() {}
}
"#,
    );

    let items = combined.extract(grep.root()).collect::<Vec<_>>();
    let names = items
      .iter()
      .map(|item| item.entry.name.as_ref())
      .collect::<Vec<_>>();

    assert_eq!(
      names,
      vec![
        "crate::api::Parser",
        "std::fmt",
        "Config",
        "Mode",
        "Config",
        "helper",
        "tests"
      ]
    );

    let config = items
      .iter()
      .find(|item| item.entry.name == "Config" && item.entry.symbol_type == SymbolType::Struct)
      .expect("Config struct should be extracted");
    let fields = config
      .members
      .iter()
      .map(|member| (member.entry.name.as_ref(), member.is_public))
      .collect::<Vec<_>>();
    assert_eq!(fields, vec![("name", true), ("enabled", false)]);

    let mode = items
      .iter()
      .find(|item| item.entry.name == "Mode")
      .expect("Mode enum should be extracted");
    let variants = mode
      .members
      .iter()
      .map(|member| (member.entry.name.as_ref(), member.entry.signature.as_ref()))
      .collect::<Vec<_>>();
    assert_eq!(
      variants,
      vec![
        ("Fast", "Fast"),
        ("Slow", "Slow"),
        ("RuleConfig", "RuleConfig"),
        ("Predicate", "Predicate"),
        ("Template", "Template"),
        ("Complex", "Complex")
      ]
    );

    let implementation = items
      .iter()
      .find(|item| item.entry.name == "Config" && item.entry.symbol_type == SymbolType::Object)
      .expect("Config impl should be extracted");
    let methods = implementation
      .members
      .iter()
      .map(|member| (member.entry.name.as_ref(), member.is_public))
      .collect::<Vec<_>>();
    assert_eq!(methods, vec![("new", true), ("enabled", false)]);

    let tests = items
      .iter()
      .find(|item| item.entry.name == "tests")
      .expect("inline test module should be extracted");
    let members = tests
      .members
      .iter()
      .map(|member| (member.entry.symbol_type, member.entry.name.as_ref()))
      .collect::<Vec<_>>();
    assert_eq!(
      members,
      vec![
        (SymbolType::Function, "nested_helper"),
        (SymbolType::Function, "parses_config")
      ]
    );
  }

  #[test]
  fn rust_builtin_rules_scope_inline_modules_and_impls() {
    let combined = rust_combined();
    let grep = SupportLang::Rust.ast_grep(
      r#"
mod tests {
  pub fn public_case() {}
  fn helper() -> bool { false }
  struct Fixture {}
  enum Mode { A }
}

trait Service {}

impl Service for Config {
  fn run(&self) {}
}

impl<T> Box<T> {
  pub fn value(&self) -> &T { todo!() }
}

impl Rewrite<String> {
  pub fn parse<L: Language>(&self, lang: &L) -> Result<Rewrite<MetaVariable>, TransformError> {
    todo!()
  }
}
"#,
    );

    let items = combined.extract(grep.root()).collect::<Vec<_>>();
    let names = items
      .iter()
      .map(|item| item.entry.name.as_ref())
      .collect::<Vec<_>>();

    assert_eq!(
      names,
      vec!["tests", "Service", "Config", "Box<T>", "Rewrite<String>"]
    );

    let tests = items
      .iter()
      .find(|item| item.entry.name == "tests")
      .expect("inline test module should be extracted");
    let module_members = tests
      .members
      .iter()
      .map(|member| (member.entry.symbol_type, member.entry.name.as_ref()))
      .collect::<Vec<_>>();
    assert_eq!(
      module_members,
      vec![
        (SymbolType::Function, "public_case"),
        (SymbolType::Function, "helper"),
        (SymbolType::Struct, "Fixture"),
        (SymbolType::Enum, "Mode")
      ]
    );

    let trait_impl = items
      .iter()
      .find(|item| item.entry.signature == "impl Service for Config")
      .expect("trait impl should be extracted");
    let trait_impl_methods = trait_impl
      .members
      .iter()
      .map(|member| member.entry.name.as_ref())
      .collect::<Vec<_>>();
    assert_eq!(trait_impl_methods, vec!["run"]);

    let generic_impl = items
      .iter()
      .find(|item| item.entry.signature == "impl<T> Box<T>")
      .expect("generic impl should be extracted");
    let generic_impl_methods = generic_impl
      .members
      .iter()
      .map(|member| member.entry.name.as_ref())
      .collect::<Vec<_>>();
    assert_eq!(generic_impl_methods, vec!["value"]);

    let rewrite_impl = items
      .iter()
      .find(|item| item.entry.signature == "impl Rewrite<String>")
      .expect("impl with type arguments should be extracted");
    let rewrite_methods = rewrite_impl
      .members
      .iter()
      .map(|member| {
        (
          member.entry.name.as_ref(),
          member.entry.signature.as_ref(),
          member.is_public,
        )
      })
      .collect::<Vec<_>>();
    assert_eq!(
      rewrite_methods,
      vec![(
        "parse",
        "pub fn parse<L: Language>(&self, lang: &L) -> Result<Rewrite<MetaVariable>, TransformError>",
        true
      )]
    );
  }

  #[test]
  fn rust_builtin_rules_extract_tokio_declaration_shapes() {
    let combined = rust_combined();
    let grep = SupportLang::Rust.ast_grep(
      r#"
pub(super) struct Cell<T: Future, S> {
  pub(super) header: Header,
  core: Core<T, S>,
}

pub(crate) struct Launch(Vec<Arc<Worker>>);

struct Local<T>(T);

pub(super) enum Scheduler<T> {
  CurrentThread(T),
  MultiThread,
}

enum Stage<T: Future> {
  Running(T),
  Finished,
}

pub fn spawn<F>(&self, future: F) -> JoinHandle<F::Output>
where
  F: Future + Send + 'static,
  F::Output: Send + 'static,
{
  todo!()
}

pub(crate) fn block_in_place<F, R>(f: F) -> R
where
  F: FnOnce() -> R,
{
  f()
}

fn with_current<R>(f: impl FnOnce(Option<&Context>) -> R) -> R {
  f(None)
}

impl<T: Future> CoreStage<T> {
  pub(super) fn with_mut<R>(&self, f: impl FnOnce(*mut Stage<T>) -> R) -> R {
    todo!()
  }

  fn with_core<F, R>(&self, f: F) -> R
  where
    F: FnOnce(&mut Core) -> R,
  {
    todo!()
  }
}
"#,
    );

    let items = combined.extract(grep.root()).collect::<Vec<_>>();
    let item_shapes = items
      .iter()
      .map(|item| {
        (
          item.entry.symbol_type,
          item.entry.name.as_ref(),
          item.is_exported,
        )
      })
      .collect::<Vec<_>>();

    assert_eq!(
      item_shapes,
      vec![
        (SymbolType::Struct, "Cell", true),
        (SymbolType::Struct, "Launch", true),
        (SymbolType::Struct, "Local", false),
        (SymbolType::Enum, "Scheduler", true),
        (SymbolType::Enum, "Stage", false),
        (SymbolType::Function, "spawn", true),
        (SymbolType::Function, "block_in_place", true),
        (SymbolType::Function, "with_current", false),
        (SymbolType::Object, "CoreStage<T>", false),
      ]
    );

    let scheduler = items
      .iter()
      .find(|item| item.entry.name == "Scheduler")
      .expect("restricted visibility generic enum should be extracted");
    let variants = scheduler
      .members
      .iter()
      .map(|member| member.entry.name.as_ref())
      .collect::<Vec<_>>();
    assert_eq!(variants, vec!["CurrentThread", "MultiThread"]);

    let implementation = items
      .iter()
      .find(|item| item.entry.name == "CoreStage<T>")
      .expect("generic impl should be extracted");
    let methods = implementation
      .members
      .iter()
      .map(|member| {
        (
          member.entry.name.as_ref(),
          member.entry.signature.as_ref(),
          member.is_public,
        )
      })
      .collect::<Vec<_>>();
    assert_eq!(
      methods,
      vec![
        (
          "with_mut",
          "pub(super) fn with_mut<R>(&self, f: impl FnOnce(*mut Stage<T>) -> R) -> R",
          true
        ),
        ("with_core", "fn with_core<F, R>(&self, f: F) -> R", false),
      ]
    );
  }

  #[test]
  fn rust_builtin_rules_extract_async_functions_and_methods() {
    let combined = rust_combined();
    let grep = SupportLang::Rust.ast_grep(
      r#"
pub async fn exported_async() -> usize { 1 }

async fn private_async<T: Send>(input: T)
where
  T: Clone,
{
  todo!()
}

mod api {
  pub async fn nested_async() {}
  async fn hidden_async() {}
}

struct Client;

impl Client {
  pub async fn connect<T>(&self, item: T) -> Result<(), ()>
  where
    T: Send,
  {
    Ok(())
  }

  async fn close(&self) {}
}

trait Service {
  async fn poll(&self);
  async fn defaulted<T>(&self) -> usize
  where
    T: Send,
  {
    1
  }
}
"#,
    );

    let items = combined.extract(grep.root()).collect::<Vec<_>>();
    let item_shapes = items
      .iter()
      .map(|item| {
        (
          item.entry.symbol_type,
          item.entry.name.as_ref(),
          item.entry.signature.as_ref(),
          item.is_exported,
        )
      })
      .collect::<Vec<_>>();

    assert_eq!(
      item_shapes,
      vec![
        (
          SymbolType::Function,
          "exported_async",
          "pub async fn exported_async() -> usize",
          true
        ),
        (
          SymbolType::Function,
          "private_async",
          "async fn private_async<T: Send>(input: T)",
          false
        ),
        (SymbolType::Module, "api", "mod api", false),
        (SymbolType::Struct, "Client", "struct Client", false),
        (SymbolType::Object, "Client", "impl Client", false),
        (SymbolType::Interface, "Service", "trait Service", false),
      ]
    );

    let api = items
      .iter()
      .find(|item| item.entry.name == "api")
      .expect("module should be extracted");
    let module_functions = api
      .members
      .iter()
      .map(|member| {
        (
          member.entry.symbol_type,
          member.entry.name.as_ref(),
          member.entry.signature.as_ref(),
          member.is_public,
        )
      })
      .collect::<Vec<_>>();
    assert_eq!(
      module_functions,
      vec![
        (
          SymbolType::Function,
          "nested_async",
          "pub async fn nested_async()",
          true
        ),
        (
          SymbolType::Function,
          "hidden_async",
          "async fn hidden_async()",
          false
        ),
      ]
    );

    let implementation = items
      .iter()
      .find(|item| item.entry.signature == "impl Client")
      .expect("impl should be extracted");
    let impl_methods = implementation
      .members
      .iter()
      .map(|member| {
        (
          member.entry.name.as_ref(),
          member.entry.signature.as_ref(),
          member.is_public,
        )
      })
      .collect::<Vec<_>>();
    assert_eq!(
      impl_methods,
      vec![
        (
          "connect",
          "pub async fn connect<T>(&self, item: T) -> Result<(), ()>",
          true
        ),
        ("close", "async fn close(&self)", false),
      ]
    );

    let service = items
      .iter()
      .find(|item| item.entry.name == "Service")
      .expect("trait should be extracted");
    let trait_methods = service
      .members
      .iter()
      .map(|member| {
        (
          member.entry.name.as_ref(),
          member.entry.signature.as_ref(),
          member.is_public,
        )
      })
      .collect::<Vec<_>>();
    assert_eq!(
      trait_methods,
      vec![
        ("poll", "async fn poll(&self)", true),
        ("defaulted", "async fn defaulted<T>(&self) -> usize", true),
      ]
    );
  }
}
