use super::indent::{extract_with_deindent, get_indent_at_offset, indent_lines, DeindentedExtract};
use super::{split_first_meta_var, MetaVarExtract, Replacer, Underlying};
use crate::language::Language;
use crate::matcher::NodeMatch;
use crate::meta_var::MetaVarEnv;
use crate::source::{Content, Doc};

use thiserror::Error;

use std::borrow::Cow;
use std::collections::HashSet;

pub enum TemplateFix {
  // no meta_var, pure text
  Textual(String),
  WithMetaVar(Template),
}

#[derive(Debug, Error)]
pub enum TemplateFixError {}

impl TemplateFix {
  pub fn try_new<L: Language>(template: &str, lang: &L) -> Result<Self, TemplateFixError> {
    Ok(create_template(template, lang.meta_var_char(), &[]))
  }

  pub fn with_transform<L: Language>(tpl: &str, lang: &L, trans: &[String]) -> Self {
    create_template(tpl, lang.meta_var_char(), trans)
  }

  pub fn used_vars(&self) -> HashSet<&str> {
    let template = match self {
      TemplateFix::WithMetaVar(t) => t,
      TemplateFix::Textual(_) => return HashSet::new(),
    };
    template.vars.iter().map(|v| v.0.used_var()).collect()
  }
}

impl<D: Doc> Replacer<D> for TemplateFix {
  fn generate_replacement(&self, nm: &NodeMatch<D>) -> Underlying<D::Source> {
    let leading = nm.root.doc.get_source().get_range(0..nm.range().start);
    let indent = get_indent_at_offset::<D::Source>(leading);
    let bytes = replace_fixer(self, nm.get_env());
    let replaced = DeindentedExtract::MultiLine(&bytes, 0);
    indent_lines::<D::Source>(indent, replaced).to_vec()
  }
}

type Indent = usize;

pub struct Template {
  fragments: Vec<String>,
  vars: Vec<(MetaVarExtract, Indent)>,
}

fn create_template(tmpl: &str, mv_char: char, transforms: &[String]) -> TemplateFix {
  let mut fragments = vec![];
  let mut vars = vec![];
  let mut offset = 0;
  let mut len = 0;
  while let Some(i) = tmpl[len + offset..].find(mv_char) {
    if let Some((meta_var, skipped)) =
      split_first_meta_var(&tmpl[len + offset + i..], mv_char, transforms)
    {
      fragments.push(tmpl[len..len + offset + i].to_string());
      // NB we have to count ident of the full string
      let indent = get_indent_at_offset::<String>(&tmpl.as_bytes()[..len + offset + i]);
      vars.push((meta_var, indent));
      len += skipped + offset + i;
      offset = 0;
      continue;
    }
    debug_assert!(len + offset + i < tmpl.len());
    // offset = 0, i = 0,
    // 0 1 2
    // $ a $
    offset = offset + i + 1;
  }
  if fragments.is_empty() {
    TemplateFix::Textual(tmpl[len..].to_string())
  } else {
    fragments.push(tmpl[len..].to_string());
    TemplateFix::WithMetaVar(Template { fragments, vars })
  }
}

fn replace_fixer<D: Doc>(
  fixer: &TemplateFix,
  env: &MetaVarEnv<D>,
) -> Vec<<D::Source as Content>::Underlying> {
  let template = match fixer {
    TemplateFix::Textual(n) => return D::Source::decode_str(n).to_vec(),
    TemplateFix::WithMetaVar(t) => t,
  };
  let mut ret = vec![];
  let mut frags = template.fragments.iter();
  let vars = template.vars.iter();
  if let Some(frag) = frags.next() {
    ret.extend_from_slice(&D::Source::decode_str(frag));
  }
  for ((var, indent), frag) in vars.zip(frags) {
    if let Some(bytes) = maybe_get_var(env, var, indent) {
      ret.extend_from_slice(&bytes);
    }
    ret.extend_from_slice(&D::Source::decode_str(frag));
  }
  ret
}

fn maybe_get_var<'e, C, D>(
  env: &'e MetaVarEnv<D>,
  var: &MetaVarExtract,
  indent: &usize,
) -> Option<Cow<'e, [C::Underlying]>>
where
  C: Content + 'e,
  D: Doc<Source = C>,
{
  let (source, range) = match var {
    MetaVarExtract::Transformed(name) => {
      // transformed source does not have range, directly return bytes
      let source = env.get_transformed(name)?;
      let de_intended = DeindentedExtract::MultiLine(source, 0);
      let bytes = indent_lines::<D::Source>(*indent, de_intended);
      return Some(bytes);
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
pub fn gen_replacement<D: Doc>(template: &str, nm: &NodeMatch<D>) -> Underlying<D::Source> {
  let fixer = create_template(template, nm.lang().meta_var_char(), &[]);
  fixer.generate_replacement(nm)
}

#[cfg(test)]
mod test {

  use super::*;
  use crate::language::{Language, Tsx};
  use crate::meta_var::{MetaVarEnv, MetaVariable};
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
      env.insert(var, root.root());
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
      env.insert_multi(var, root.root().children().collect());
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
      env.insert(var, root.root());
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
  fn test_template_vars() {
    let tf = TemplateFix::try_new("$A $B $C", &Tsx).expect("ok");
    assert_eq!(tf.used_vars(), ["A", "B", "C"].into_iter().collect());
    let tf = TemplateFix::try_new("$a$B$C", &Tsx).expect("ok");
    assert_eq!(tf.used_vars(), ["B", "C"].into_iter().collect());
    let tf = TemplateFix::try_new("$a$B$C", &Tsx).expect("ok");
    assert_eq!(tf.used_vars(), ["B", "C"].into_iter().collect());
  }

  // GH #641
  #[test]
  fn test_multi_row_replace() {
    test_template_replace(
      "$A = $B",
      &[("A", "x"), ("B", "[\n  1\n]")],
      "x = [\n  1\n]",
    );
  }

  #[test]
  fn test_replace_rewriter() {
    let tf = TemplateFix::with_transform("if (a)\n  $A", &Tsx, &["A".to_string()]);
    let mut env = MetaVarEnv::new();
    env.insert_transformation(
      &MetaVariable::Multiple,
      "A",
      "if (b)\n  foo".bytes().collect(),
    );
    let dummy = Tsx.ast_grep("dummy");
    let node_match = NodeMatch::new(dummy.root(), env.clone());
    let bytes = tf.generate_replacement(&node_match);
    let ret = String::from_utf8(bytes).expect("replacement must be valid utf-8");
    assert_eq!("if (a)\n  if (b)\n    foo", ret);
  }

  #[test]
  fn test_nested_matching_replace() {
    // TODO impossible, we don't support nested replacement
  }
}
