use super::Ctx;

use crate::fixer::{Fixer, SerializableFixer};
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
    let bytes = ctx.env.get_var_bytes(&var)?;
    let rules: Vec<_> = self
      .rewrites
      .iter()
      .filter_map(|id| ctx.rewriters.get(id))
      .collect();
    let edits = find_and_make_edits(nodes, &rules);
    let rewritten = make_edit::<D>(bytes, edits);
    Some(D::Source::encode_bytes(rewritten).to_string())
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
        let deserialize_env = rule.get_env(node.lang().clone());
        todo!()
        // let fixer = Fixer::parse(fixer, &deserialize_env, &Default::default()).unwrap();
        // edits.push(nm.make_edit(rule, &fixer));
      }
    }
  }
  edits
}

fn make_edit<D: Doc>(bytes: &Bytes<D>, edits: Vec<Edit<D::Source>>) -> &Bytes<D> {
  todo!()
}
