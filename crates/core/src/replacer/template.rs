use crate::language::Language;
use crate::meta_var::{split_first_meta_var, MetaVarEnv};
use crate::source::{Content, Doc, StrDoc};
use std::borrow::Cow;

enum Fixer<'a, C: Content> {
  // no meta_var, pure text
  Textual(Cow<'a, [C::Underlying]>),
  WithMetaVar(Template<'a, C>),
}

struct Template<'a, C: Content> {
  fragments: Vec<Cow<'a, [C::Underlying]>>,
  vars: Vec<&'a str>,
}

fn create_fixer<C: Content>(mut template: &str, mv_char: char) -> Fixer<C> {
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

fn replace_fixer<'a, D: Doc>(
  fixer: &Fixer<'a, D::Source>,
  env: &MetaVarEnv<D>,
) -> Cow<'a, [<D::Source as Content>::Underlying]> {
  let template = match fixer {
    Fixer::Textual(n) => return n.clone(),
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
      let bytes = <D::Source as Content>::decode_str(&text);
      ret.extend_from_slice(&bytes);
    }
    ret.extend_from_slice(frag);
  }
  Cow::Owned(ret)
}

// replace meta_var in template string, e.g. "Hello $NAME" -> "Hello World"
// use Cow instead of String to avoid allocation
pub fn replace_meta_var_in_string<'a, L: Language>(
  template: &'a str,
  env: &MetaVarEnv<StrDoc<L>>,
  lang: &L,
) -> Cow<'a, str> {
  let fixer = create_fixer(template, lang.meta_var_char());
  match replace_fixer(&fixer, env) {
    Cow::Borrowed(n) => Cow::Borrowed(unsafe { std::str::from_utf8_unchecked(n) }),
    Cow::Owned(n) => Cow::Owned(unsafe { String::from_utf8_unchecked(n) }),
  }
}
