use super::indent::IndentSensitive;
use super::Underlying;
use crate::language::Language;
use crate::matcher::NodeMatch;
use crate::meta_var::{split_first_meta_var, MatchResult, MetaVarEnv, MetaVariable};
use crate::source::{Content, Doc};
use std::borrow::Cow;

// TODO: this should be public
enum Fixer<'a, C: IndentSensitive> {
  // no meta_var, pure text
  Textual(Cow<'a, [C::Underlying]>),
  WithMetaVar(Template<'a, C>),
}

struct Template<'a, C: IndentSensitive> {
  fragments: Vec<Cow<'a, [C::Underlying]>>,
  vars: Vec<MetaVariable>,
}

fn create_fixer<C: IndentSensitive>(mut template: &str, mv_char: char) -> Fixer<C> {
  let mut fragments = vec![];
  let mut vars = vec![];
  let mut offset = 0;
  while let Some(i) = template[offset..].find(mv_char) {
    if let Some((meta_var, remaining)) = split_first_meta_var(&template[offset + i..], mv_char) {
      fragments.push(C::decode_str(&template[..offset + i]));
      vars.push(meta_var);
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
  for (var, frag) in vars.zip(frags) {
    if let Some(matched) = env.get(var) {
      // TODO: abstract this with structral
      let bytes = match matched {
        MatchResult::Single(replaced) => replaced
          .root
          .doc
          .get_source()
          .get_range(replaced.range())
          .to_vec(),
        MatchResult::Multi(nodes) => {
          if nodes.is_empty() {
            vec![]
          } else {
            // NOTE: start_byte is not always index range of source's slice.
            // e.g. start_byte is still byte_offset in utf_16 (napi). start_byte
            // so we need to call source's get_range method
            let start = nodes[0].inner.start_byte() as usize;
            let end = nodes[nodes.len() - 1].inner.end_byte() as usize;
            nodes[0]
              .root
              .doc
              .get_source()
              .get_range(start..end)
              .to_vec()
          }
        }
      };
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
