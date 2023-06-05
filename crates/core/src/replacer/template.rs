use super::indent::IndentSensitive;
use super::Underlying;
use crate::language::Language;
use crate::matcher::NodeMatch;
use crate::meta_var::{split_first_meta_var, MetaVarEnv};
use crate::source::{Content, Doc};
use std::borrow::Cow;

enum Fixer<'a, C: IndentSensitive> {
  // no meta_var, pure text
  Textual(Cow<'a, [C::Underlying]>),
  WithMetaVar(Template<'a, C>),
}

struct Template<'a, C: IndentSensitive> {
  fragments: Vec<Cow<'a, [C::Underlying]>>,
  vars: Vec<&'a str>,
}

fn create_fixer<C: IndentSensitive>(mut template: &str, mv_char: char) -> Fixer<C> {
  let mut fragments = vec![];
  let mut vars = vec![];
  while let Some(i) = template.find(mv_char) {
    fragments.push(C::decode_str(&template[..i]));
    template = &template[i..];
    let (meta_var, remaining) = split_first_meta_var(template, mv_char);
    vars.push(meta_var);
    template = remaining;
  }
  if fragments.is_empty() {
    Fixer::Textual(C::decode_str(template))
  } else {
    fragments.push(C::decode_str(template));
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
  // TODO: fix this get single var only
  for (var, frag) in vars.zip(frags) {
    if let Some(node) = env.get_match(var) {
      // TODO: add indentation
      let text = node.text();
      let bytes = <D::Source as IndentSensitive>::decode_str(&text);
      ret.extend_from_slice(&bytes);
    }
    ret.extend_from_slice(frag);
  }
  ret
}

// replace meta_var in template string, e.g. "Hello $NAME" -> "Hello World"
pub fn gen_replacement<D: Doc>(template: &str, nm: &NodeMatch<D>) -> Underlying<D::Source>
where
  D::Source: IndentSensitive,
{
  let fixer = create_fixer(template, nm.lang().meta_var_char());
  replace_fixer(&fixer, nm.get_env())
}
