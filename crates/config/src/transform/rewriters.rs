use super::Ctx;
use crate::rule_core::RuleCore;

use ast_grep_core::meta_var::MetaVariable;
use ast_grep_core::source::{Content, Edit};
use ast_grep_core::{Doc, Language, Matcher, Node};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Rewriters {
  source: String,
  rewrites: Vec<String>,
  // do we need this?
  // sort_by: Option<String>,
  join_by: Option<String>,
}

fn get_nodes_from_env<'b, D: Doc>(var: &MetaVariable, ctx: &Ctx<'_, 'b, D>) -> Vec<Node<'b, D>> {
  match var {
    MetaVariable::MultiCapture(n) => ctx.env.get_multiple_matches(n),
    MetaVariable::Capture(m, _) => {
      if let Some(n) = ctx.env.get_match(m) {
        vec![n.clone()]
      } else {
        vec![]
      }
    }
    _ => vec![],
  }
}

impl Rewriters {
  pub(super) fn compute<D: Doc>(&self, ctx: &mut Ctx<D>) -> Option<String> {
    let source = ctx.lang.pre_process_pattern(&self.source);
    let var = ctx.lang.extract_meta_var(&source)?;
    let nodes = get_nodes_from_env(&var, ctx);
    if nodes.is_empty() {
      return None;
    }
    let start = nodes[0].range().start;
    let bytes = ctx.env.get_var_bytes(&var)?;
    let rules: Vec<_> = self
      .rewrites
      .iter()
      .filter_map(|id| ctx.rewriters.get(id))
      .collect();
    let edits = find_and_make_edits(nodes, &rules);
    let rewritten = if let Some(joiner) = &self.join_by {
      let mut ret = vec![];
      let mut edits = edits.into_iter();
      if let Some(first) = edits.next() {
        ret.extend(first.inserted_text);
        let joiner = D::Source::decode_str(joiner);
        for edit in edits {
          ret.extend_from_slice(&joiner);
          ret.extend(edit.inserted_text);
        }
        ret
      } else {
        ret
      }
    } else {
      make_edit::<D>(bytes, edits, start)
    };
    Some(D::Source::encode_bytes(&rewritten).to_string())
  }
}

type Bytes<D> = [<<D as Doc>::Source as Content>::Underlying];
fn find_and_make_edits<D: Doc>(
  nodes: Vec<Node<D>>,
  rules: &[&RuleCore<D::Lang>],
) -> Vec<Edit<D::Source>> {
  nodes
    .into_iter()
    .flat_map(|n| replace_one(n, rules))
    .collect()
}

fn replace_one<D: Doc>(node: Node<D>, rules: &[&RuleCore<D::Lang>]) -> Vec<Edit<D::Source>> {
  let mut edits = vec![];
  for child in node.dfs() {
    for rule in rules {
      // TODO inherit deserialize_env and meta_var_env
      if let Some(nm) = rule.match_node(child.clone()) {
        edits.push(nm.make_edit(rule, rule.fixer.as_ref().expect("TODO")));
      }
    }
  }
  edits
}

fn make_edit<D: Doc>(
  old_content: &Bytes<D>,
  edits: Vec<Edit<D::Source>>,
  offset: usize,
) -> Vec<<<D as Doc>::Source as Content>::Underlying> {
  let mut new_content = vec![];
  let mut start = 0;
  for edit in edits {
    let pos = edit.position - offset;
    new_content.extend_from_slice(&old_content[start..pos]);
    new_content.extend_from_slice(&edit.inserted_text);
    start = pos + edit.deleted_length;
  }
  // add trailing statements
  new_content.extend_from_slice(&old_content[start..]);
  new_content
}

#[cfg(test)]
mod test {
  // test applyRewriters actually works given correct env and rewriters
}
