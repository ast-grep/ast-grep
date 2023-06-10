use super::indent::{
  extract_with_deindent, get_indent_at_offset, indent_lines, DeindentedExtract, IndentSensitive,
};
use super::{split_first_meta_var, MetaVarExtract, Replacer, Underlying};
use crate::language::Language;
use crate::matcher::NodeMatch;
use crate::meta_var::MetaVarEnv;
use crate::source::{Content, Doc};

use std::borrow::Cow;
use thiserror::Error;

pub enum Fixer<C: IndentSensitive> {
  // no meta_var, pure text
  Textual(Vec<C::Underlying>),
  WithMetaVar(Template<C>),
}

#[derive(Debug, Error)]
pub enum FixerError {}

impl<C: IndentSensitive> Fixer<C> {
  pub fn try_new<L: Language>(template: &str, lang: &L) -> Result<Self, FixerError> {
    Ok(create_fixer(template, lang.meta_var_char(), &[]))
  }

  pub fn with_transform<L: Language>(tpl: &str, lang: &L, trans: &[String]) -> Self {
    create_fixer(tpl, lang.meta_var_char(), trans)
  }
}

impl<C, D> Replacer<D> for Fixer<C>
where
  C: IndentSensitive,
  D: Doc<Source = C>,
{
  fn generate_replacement(&self, nm: &NodeMatch<D>) -> Underlying<D::Source> {
    let leading = nm.root.doc.get_source().get_range(0..nm.range().start);
    let indent = get_indent_at_offset::<D::Source>(leading);
    let bytes = replace_fixer(self, nm.get_env());
    let replaced = DeindentedExtract::MultiLine(&bytes, 0);
    indent_lines::<D::Source>(indent, replaced).to_vec()
  }
}

type Indent = usize;

pub struct Template<C: IndentSensitive> {
  fragments: Vec<Vec<C::Underlying>>,
  vars: Vec<(MetaVarExtract, Indent)>,
}

fn create_fixer<C: IndentSensitive>(
  mut template: &str,
  mv_char: char,
  transforms: &[String],
) -> Fixer<C> {
  let mut fragments = vec![];
  let mut vars = vec![];
  let mut offset = 0;
  while let Some(i) = template[offset..].find(mv_char) {
    if let Some((meta_var, remaining)) =
      split_first_meta_var(&template[offset + i..], mv_char, transforms)
    {
      fragments.push(C::decode_str(&template[..offset + i]).into_owned());
      let indent = get_indent_at_offset::<String>(template[..offset + i].as_bytes());
      vars.push((meta_var, indent));
      template = remaining;
      offset = 0;
      continue;
    }
    debug_assert!(offset + i < template.len());
    // offset = 0, i = 0,
    // 0 1 2
    // $ a $
    offset = offset + i + 1;
  }
  if fragments.is_empty() {
    Fixer::Textual(C::decode_str(template).into_owned())
  } else {
    fragments.push(C::decode_str(template).into_owned());
    Fixer::WithMetaVar(Template { fragments, vars })
  }
}

fn replace_fixer<D: Doc>(
  fixer: &Fixer<D::Source>,
  env: &MetaVarEnv<D>,
) -> Vec<<D::Source as Content>::Underlying>
where
  D::Source: IndentSensitive,
{
  let template = match fixer {
    Fixer::Textual(n) => return n.to_vec(),
    Fixer::WithMetaVar(t) => t,
  };
  let mut ret = vec![];
  let mut frags = template.fragments.iter();
  let vars = template.vars.iter();
  if let Some(frag) = frags.next() {
    ret.extend_from_slice(frag);
  }
  for ((var, indent), frag) in vars.zip(frags) {
    if let Some(bytes) = maybe_get_var(env, var, indent) {
      ret.extend_from_slice(&bytes);
    }
    ret.extend_from_slice(frag);
  }
  ret
}

fn maybe_get_var<'e, C, D>(
  env: &'e MetaVarEnv<D>,
  var: &MetaVarExtract,
  indent: &usize,
) -> Option<Cow<'e, [C::Underlying]>>
where
  C: IndentSensitive + 'e,
  D: Doc<Source = C>,
{
  let (source, range) = match var {
    MetaVarExtract::Transformed(name) => {
      let source = env.get_transformed(name)?;
      return Some(Cow::Borrowed(source));
    }
    MetaVarExtract::Single(name) => {
      let replaced = env.get_match(name)?;
      let source = replaced.root.doc.get_source();
      let range = replaced.range();
      (source, range)
    }
    MetaVarExtract::Multiple(name) => {
      let nodes = env.get_multiple_matches(name);
      if nodes.is_empty() {
        return None;
      }
      // NOTE: start_byte is not always index range of source's slice.
      // e.g. start_byte is still byte_offset in utf_16 (napi). start_byte
      // so we need to call source's get_range method
      let start = nodes[0].inner.start_byte() as usize;
      let end = nodes[nodes.len() - 1].inner.end_byte() as usize;
      let source = nodes[0].root.doc.get_source();
      (source, start..end)
    }
  };
  let extracted = extract_with_deindent(source, range);
  let bytes = indent_lines::<D::Source>(*indent, extracted);
  Some(bytes)
}

// replace meta_var in template string, e.g. "Hello $NAME" -> "Hello World"
pub fn gen_replacement<D: Doc>(template: &str, nm: &NodeMatch<D>) -> Underlying<D::Source>
where
  D::Source: IndentSensitive,
{
  let fixer = create_fixer(template, nm.lang().meta_var_char(), &[]);
  fixer.generate_replacement(nm)
}

#[cfg(test)]
mod test {

  use super::*;
  use crate::language::{Language, Tsx};
  use crate::meta_var::MetaVarEnv;
  use crate::Pattern;
  use std::collections::HashMap;

  #[test]
  fn test_example() {
    let src = r"
if (true) {
  a(
    1
      + 2
      + 3
  )
}";
    let pattern = "a($B)";
    let template = r"c(
  $B
)";
    let mut src = Tsx.ast_grep(src);
    let pattern = Pattern::str(pattern, Tsx);
    let success = src.replace(pattern, template).expect("should replace");
    assert!(success);
    let expect = r"if (true) {
  c(
    1
      + 2
      + 3
  )
}";
    assert_eq!(src.root().text(), expect);
  }

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
