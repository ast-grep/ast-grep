use super::indent::{extract_with_deindent, get_indent_at_offset, indent_lines, IndentSensitive};
use super::{Replacer, Underlying};
use crate::language::Language;
use crate::matcher::NodeMatch;
use crate::meta_var::{split_first_meta_var, MatchResult, MetaVarEnv, MetaVariable};
use crate::source::{Content, Doc};

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
    Ok(create_fixer(template, lang.meta_var_char()))
  }
}

impl<C, D> Replacer<D> for Fixer<C>
where
  C: IndentSensitive,
  D: Doc<Source = C>,
{
  fn generate_replacement(&self, nm: &NodeMatch<D>) -> Underlying<D::Source> {
    replace_fixer(self, nm.get_env())
  }
}

type Indent = Option<usize>;

pub struct Template<C: IndentSensitive> {
  fragments: Vec<Vec<C::Underlying>>,
  vars: Vec<(MetaVariable, Indent)>,
}

fn create_fixer<C: IndentSensitive>(mut template: &str, mv_char: char) -> Fixer<C> {
  let mut fragments = vec![];
  let mut vars = vec![];
  let mut offset = 0;
  while let Some(i) = template[offset..].find(mv_char) {
    if let Some((meta_var, remaining)) = split_first_meta_var(&template[offset + i..], mv_char) {
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
    if let Some(matched) = env.get(var) {
      // TODO: abstract this with structral
      let (source, range) = match matched {
        MatchResult::Single(replaced) => {
          let source = replaced.root.doc.get_source();
          let range = replaced.range();
          (source, range)
        }
        MatchResult::Multi(nodes) => {
          if nodes.is_empty() {
            continue;
          } else {
            // NOTE: start_byte is not always index range of source's slice.
            // e.g. start_byte is still byte_offset in utf_16 (napi). start_byte
            // so we need to call source's get_range method
            let start = nodes[0].inner.start_byte() as usize;
            let end = nodes[nodes.len() - 1].inner.end_byte() as usize;
            let source = nodes[0].root.doc.get_source();
            (source, start..end)
          }
        }
      };
      let extracted = extract_with_deindent(source, range.clone());
      let bytes = if let Some(ext) = extracted {
        // TODO: we should indent according to the template...
        indent_lines::<D::Source>(*indent, ext)
      } else {
        source.get_range(range).to_vec()
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
  let leading = nm.root.doc.get_source().get_range(0..nm.range().start);
  let indent = get_indent_at_offset::<D::Source>(leading);
  let bytes = replace_fixer(&fixer, nm.get_env());
  let lines: Vec<_> = bytes.split(|b| *b == D::Source::NEW_LINE).collect();
  indent_lines::<D::Source>(indent, lines)
}

#[cfg(test)]
mod test {
  use crate::language::{Language, Tsx};
  use crate::Pattern;

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
}
