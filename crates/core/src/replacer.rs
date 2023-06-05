use crate::matcher::NodeMatch;
use crate::source::{Content, Edit as E};
use crate::Pattern;
use crate::{Doc, Node};

type Edit<D> = E<<D as Doc>::Source>;
type Underlying<S> = Vec<<S as Content>::Underlying>;

mod indent;
mod structural;
mod template;

pub use indent::IndentSensitive;

/// Replace meta variable in the replacer string
pub trait Replacer<D: Doc> {
  fn generate_replacement(&self, nm: &NodeMatch<D>) -> Underlying<D::Source>;
}

impl<D: Doc> Replacer<D> for str
where
  D::Source: indent::IndentSensitive,
{
  fn generate_replacement(&self, nm: &NodeMatch<D>) -> Underlying<D::Source> {
    template::gen_replacement(self, nm)
  }
}

impl<D: Doc> Replacer<D> for Pattern<D> {
  fn generate_replacement(&self, nm: &NodeMatch<D>) -> Underlying<D::Source> {
    structural::gen_replacement(&self.root, nm)
  }
}

impl<D, T> Replacer<D> for &T
where
  D: Doc,
  T: Replacer<D> + ?Sized,
{
  fn generate_replacement(&self, nm: &NodeMatch<D>) -> Underlying<D::Source> {
    (**self).generate_replacement(nm)
  }
}

impl<'a, D: Doc> Replacer<D> for Node<'a, D> {
  fn generate_replacement(&self, _nm: &NodeMatch<D>) -> Underlying<D::Source> {
    let range = self.range();
    self.root.doc.get_source().get_range(range).to_vec()
  }
}

#[cfg(test)]
mod test {
  use super::*;
  use crate::language::{Language, Tsx};
  use crate::meta_var::MetaVarEnv;
  use std::collections::HashMap;

  fn test_str_replace(replacer: &str, vars: &[(&str, &str)], expected: &str) {
    let mut env = MetaVarEnv::new();
    let roots: Vec<_> = vars
      .iter()
      .map(|(v, p)| (v, Tsx.ast_grep(p).inner))
      .collect();
    for (var, root) in &roots {
      env.insert(var.to_string(), root.root());
    }
    let dummy = Tsx.ast_grep("dummy");
    let node_match = NodeMatch::new(dummy.root(), env.clone());
    let replaced = replacer.generate_replacement(&node_match);
    let replaced = String::from_utf8_lossy(&replaced);
    assert_eq!(
      replaced,
      expected,
      "wrong replacement {replaced} {expected} {:?}",
      HashMap::from(env)
    );
  }

  #[test]
  fn test_no_env() {
    test_str_replace("let a = 123", &[], "let a = 123");
    test_str_replace(
      "console.log('hello world'); let b = 123;",
      &[],
      "console.log('hello world'); let b = 123;",
    );
  }

  #[test]
  fn test_single_env() {
    test_str_replace("let a = $A", &[("A", "123")], "let a = 123");
    test_str_replace(
      "console.log($HW); let b = 123;",
      &[("HW", "'hello world'")],
      "console.log('hello world'); let b = 123;",
    );
  }

  #[test]
  fn test_multiple_env() {
    test_str_replace("let $V = $A", &[("A", "123"), ("V", "a")], "let a = 123");
    test_str_replace(
      "console.log($HW); let $B = 123;",
      &[("HW", "'hello world'"), ("B", "b")],
      "console.log('hello world'); let b = 123;",
    );
  }

  #[test]
  fn test_multiple_occurrences() {
    test_str_replace("let $A = $A", &[("A", "a")], "let a = a");
    test_str_replace("var $A = () => $A", &[("A", "a")], "var a = () => a");
    test_str_replace(
      "const $A = () => { console.log($B); $A(); };",
      &[("B", "'hello world'"), ("A", "a")],
      "const a = () => { console.log('hello world'); a(); };",
    );
  }

  fn test_ellipsis_replace(replacer: &str, vars: &[(&str, &str)], expected: &str) {
    let mut env = MetaVarEnv::new();
    let roots: Vec<_> = vars
      .iter()
      .map(|(v, p)| (v, Tsx.ast_grep(p).inner))
      .collect();
    for (var, root) in &roots {
      env.insert_multi(var.to_string(), root.root().children().collect());
    }
    let dummy = Tsx.ast_grep("dummy");
    let node_match = NodeMatch::new(dummy.root(), env.clone());
    let replaced = replacer.generate_replacement(&node_match);
    let replaced = String::from_utf8_lossy(&replaced);
    assert_eq!(
      replaced,
      expected,
      "wrong replacement {replaced} {expected} {:?}",
      HashMap::from(env)
    );
  }

  // TODO
  #[ignore]
  #[test]
  fn test_ellipsis_meta_var() {
    test_ellipsis_replace(
      "let a = () => { $$$B }",
      &[("B", "alert('works!')")],
      "let a = () => { alert('works!') }",
    );
    test_ellipsis_replace(
      "let a = () => { $$$B }",
      &[("B", "alert('works!');console.log(123)")],
      "let a = () => { alert('works!');console.log(123) }",
    );
  }

  // TODO
  #[ignore]
  #[test]
  fn test_multi_ellipsis() {
    test_ellipsis_replace(
      "import {$$$A, B, $$$C} from 'a'",
      &[("A", "A"), ("C", "C")],
      "import {A, B, C} from 'a'",
    );
  }

  #[test]
  fn test_replace_in_string() {
    test_str_replace("'$A'", &[("A", "123")], "'123'");
  }

  fn test_template_replace(template: &str, vars: &[(&str, &str)], expected: &str) {
    let mut env = MetaVarEnv::new();
    let roots: Vec<_> = vars
      .iter()
      .map(|(v, p)| (v, Tsx.ast_grep(p).inner))
      .collect();
    for (var, root) in &roots {
      env.insert(var.to_string(), root.root());
    }
    let dummy = Tsx.ast_grep("dummy");
    let node_match = NodeMatch::new(dummy.root(), env.clone());
    let bytes = template.generate_replacement(&node_match);
    let ret = String::from_utf8(bytes).expect("replacement must be valid utf-8");
    assert_eq!(expected, ret);
  }

  #[test]
  fn test_template() {
    test_template_replace("Hello $A", &[("A", "World")], "Hello World");
    test_template_replace("$B $A", &[("A", "World"), ("B", "Hello")], "Hello World");
  }

  #[test]
  fn test_nested_matching_replace() {
    // TODO
  }
}
