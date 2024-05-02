use super::Ctx;
use crate::rule_core::RuleCore;

use ast_grep_core::meta_var::MetaVariable;
use ast_grep_core::source::{Content, Edit};
use ast_grep_core::{Doc, Language, Node, NodeMatch};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Rewrite {
  pub(super) source: String,
  pub(super) rewriters: Vec<String>,
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

impl Rewrite {
  pub(super) fn compute<D: Doc>(&self, ctx: &mut Ctx<D>) -> Option<String> {
    let source = ctx.lang.pre_process_pattern(&self.source);
    let var = ctx.lang.extract_meta_var(&source)?;
    let nodes = get_nodes_from_env(&var, ctx);
    if nodes.is_empty() {
      return None;
    }
    let rewriters = ctx.rewriters.read();
    let start = nodes[0].range().start;
    let bytes = ctx.env.get_var_bytes(&var)?;
    let rules: Vec<_> = self
      .rewriters
      .iter()
      .filter_map(|id| rewriters.get(id)) // TODO: better handling
      .collect();
    let edits = find_and_make_edits(nodes, &rules, ctx);
    let rewritten = if let Some(joiner) = &self.join_by {
      let mut ret = vec![];
      let mut edits = edits.into_iter();
      if let Some(first) = edits.next() {
        let mut pos = first.position - start + first.deleted_length;
        ret.extend(first.inserted_text);
        let joiner = D::Source::decode_str(joiner);
        for edit in edits {
          let p = edit.position - start;
          // skip overlapping edits
          if pos > p {
            continue;
          }
          ret.extend_from_slice(&joiner);
          ret.extend(edit.inserted_text);
          pos = p + edit.deleted_length;
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
fn find_and_make_edits<'n, D: Doc>(
  nodes: Vec<Node<'n, D>>,
  rules: &[&RuleCore<D::Lang>],
  ctx: &Ctx<'_, 'n, D>,
) -> Vec<Edit<D::Source>> {
  nodes
    .into_iter()
    .flat_map(|n| replace_one(n, rules, ctx))
    .collect()
}

fn replace_one<'n, D: Doc>(
  node: Node<'n, D>,
  rules: &[&RuleCore<D::Lang>],
  ctx: &Ctx<'_, 'n, D>,
) -> Vec<Edit<D::Source>> {
  let mut edits = vec![];
  for child in node.dfs() {
    for rule in rules {
      let mut env = std::borrow::Cow::Borrowed(ctx.enclosing_env);
      // NOTE: we inherit meta_var_env from enclosing rule
      // but match env will NOT inherited recursively!
      // e.g. $B is matched in parent linter and it is inherited.
      // $C is matched in rewriter but is NOT inherited in recursive rewriter
      // this is to enable recursive rewriter to match sub nodes
      // in future, we can use the explict `expose` to control env inheritance
      if let Some(n) = rule.do_match(child.clone(), &mut env, Some(ctx.enclosing_env)) {
        let nm = NodeMatch::new(n, env.into_owned());
        edits.push(nm.make_edit(rule, rule.fixer.as_ref().expect("TODO")));
        // stop at first fix, skip duplicate fix
        break;
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
    // skip overlapping edits
    if start > pos {
      continue;
    }
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
  use super::*;
  use crate::from_str;
  use crate::rule::DeserializeEnv;
  use crate::rule_core::SerializableRuleCore;
  use crate::test::TypeScript;
  use crate::GlobalRules;

  fn apply_transformation(
    rewrite: Rewrite,
    src: &str,
    pat: &str,
    rewriters: GlobalRules<TypeScript>,
  ) -> String {
    let grep = TypeScript::Tsx.ast_grep(src);
    let root = grep.root();
    let mut nm = root.find(pat).expect("should find");
    let before_vars: Vec<_> = nm.get_env().get_matched_variables().collect();
    let env = nm.get_env_mut();
    let enclosing = env.clone();
    let mut ctx = Ctx {
      lang: &TypeScript::Tsx,
      transforms: &Default::default(),
      env,
      rewriters,
      enclosing_env: &enclosing,
    };
    let after_vars: Vec<_> = ctx.env.get_matched_variables().collect();
    assert_eq!(
      before_vars, after_vars,
      "rewrite should not write back to env"
    );
    rewrite.compute(&mut ctx).expect("should have transforms")
  }
  macro_rules! str_vec {
    ( $($a: expr),* ) => { vec![ $($a.to_string()),* ] };
  }

  fn make_rewriter(pairs: &[(&str, &str)]) -> GlobalRules<TypeScript> {
    let rewriters = GlobalRules::default();
    for (key, ser) in pairs {
      let serialized: SerializableRuleCore = from_str(ser).unwrap();
      let env = DeserializeEnv::new(TypeScript::Tsx).with_rewriters(&rewriters);
      let rule = serialized.get_rewriter(env, &Default::default()).unwrap();
      rewriters.insert(key, rule).unwrap();
    }
    rewriters
  }

  #[test]
  fn test_perform_one_rewrite() {
    let rewrite = Rewrite {
      source: "$A".into(),
      rewriters: str_vec!["rewrite"],
      join_by: None,
    };
    let rewriters = make_rewriter(&[("rewrite", "{rule: {kind: number}, fix: '810'}")]);
    let ret = apply_transformation(rewrite, "log(t(1, 2, 3))", "log($A)", rewriters);
    assert_eq!(ret, "t(810, 810, 810)");
  }

  #[test]
  fn test_perform_multiple_rewriters() {
    let rewrite = Rewrite {
      source: "$A".into(),
      rewriters: str_vec!["re1", "re2"],
      join_by: None,
    };
    let rewriters = make_rewriter(&[
      ("re1", "{rule: {regex: '^1$'}, fix: '810'}"),
      ("re2", "{rule: {regex: '^2$'}, fix: '1919'}"),
    ]);
    let ret = apply_transformation(rewrite, "log(t(1, 2, 3))", "log($A)", rewriters);
    assert_eq!(ret, "t(810, 1919, 3)");
  }

  #[test]
  fn test_ignore_unused_rewriters() {
    let rewrite = Rewrite {
      source: "$A".into(),
      rewriters: str_vec!["re1"],
      join_by: None,
    };
    let rewriters = make_rewriter(&[
      ("ignored", "{rule: {regex: '^2$'}, fix: '1919'}"),
      ("re1", "{rule: {kind: number}, fix: '810'}"),
    ]);
    let ret = apply_transformation(rewrite, "log(t(1, 2, 3))", "log($A)", rewriters);
    assert_eq!(ret, "t(810, 810, 810)");
  }

  #[test]
  fn test_rewriters_order() {
    let rewrite = Rewrite {
      source: "$A".into(),
      rewriters: str_vec!["re2", "re1"],
      join_by: None,
    };
    // first match wins the rewrite
    let rewriters = make_rewriter(&[
      ("re2", "{rule: {regex: '^2$'}, fix: '1919'}"),
      ("re1", "{rule: {kind: number}, fix: '810'}"),
    ]);
    let ret = apply_transformation(rewrite, "log(t(1, 2, 3))", "log($A)", rewriters);
    assert_eq!(ret, "t(810, 1919, 810)");
  }

  #[test]
  fn test_rewriters_overlapping() {
    let rewrite = Rewrite {
      source: "$A".into(),
      rewriters: str_vec!["re1", "re2"],
      join_by: None,
    };
    // parent node wins fix, even if rule comes later
    let rewriters = make_rewriter(&[
      ("re1", "{rule: {kind: number}, fix: '810'}"),
      ("re2", "{rule: {kind: array}, fix: '1919'}"),
    ]);
    let ret = apply_transformation(rewrite, "[1, 2, 3]", "$A", rewriters);
    assert_eq!(ret, "1919");
  }

  #[test]
  fn test_rewriters_join_by() {
    let rewrite = Rewrite {
      source: "$A".into(),
      rewriters: str_vec!["re1"],
      join_by: Some(" + ".into()),
    };
    let rewriters = make_rewriter(&[("re1", "{rule: {kind: number}, fix: '810'}")]);
    let ret = apply_transformation(rewrite, "log(t(1, 2, 3))", "log($A)", rewriters);
    assert_eq!(ret, "810 + 810 + 810");
  }

  #[test]
  fn test_recursive_rewriters() {
    let rewrite = Rewrite {
      source: "$A".into(),
      rewriters: str_vec!["re1"],
      join_by: None,
    };
    let rule = r#"
rule: {pattern: '[$$$C]'}
transform:
  D:
    rewrite:
      source: $$$C
      rewriters: [re1]
fix: $D
    "#;
    let rewriters = make_rewriter(&[("re1", rule)]);
    let ret = apply_transformation(rewrite, "[1, [2, [3, [4]]]]", "$A", rewriters);
    assert_eq!(ret, "1, 2, 3, 4");
  }

  #[test]
  fn test_should_inherit_match_env() {
    let rewrite = Rewrite {
      source: "$A".into(),
      rewriters: str_vec!["re"],
      join_by: None,
    };
    let rewriters = make_rewriter(&[("re", "{rule: {pattern: $C}, fix: '123'}")]);
    let ret = apply_transformation(rewrite.clone(), "[1, 2]", "[$A, $B]", rewriters.clone());
    assert_eq!(ret, "123");
    let ret = apply_transformation(rewrite.clone(), "[1, 1]", "[$A, $C]", rewriters.clone());
    assert_eq!(ret, "123");
    // should not match $C so no rewrite
    let ret = apply_transformation(rewrite, "[1, 2]", "[$A, $C]", rewriters);
    assert_eq!(ret, "1");
  }

  #[test]
  fn test_node_not_found() {
    let rewrite = Rewrite {
      source: "$A".into(),
      rewriters: str_vec!["re"],
      join_by: None,
    };
    let rewriters = make_rewriter(&[("re", "{rule: {pattern: $B}, fix: '123'}")]);
    let grep = TypeScript::Tsx.ast_grep("[1, 2]");
    let root = grep.root();
    let mut nm = root.find("[$B, $C]").expect("should find");
    let env = nm.get_env_mut();
    let enclosing = env.clone();
    let mut ctx = Ctx {
      lang: &TypeScript::Tsx,
      transforms: &Default::default(),
      env,
      rewriters,
      enclosing_env: &enclosing,
    };
    let ret = rewrite.compute(&mut ctx);
    assert_eq!(ret, None);
  }

  #[test]
  fn test_rewrite_use_enclosing_env() {
    let rewrite = Rewrite {
      source: "$A".into(),
      rewriters: str_vec!["re"],
      join_by: None,
    };
    let rewriters = GlobalRules::default();
    let serialized: SerializableRuleCore =
      from_str("{rule: {pattern: $B}, fix: '$B == $C'}").unwrap();
    let env = DeserializeEnv::new(TypeScript::Tsx).with_rewriters(&rewriters);
    let mut vars = std::collections::HashSet::new();
    vars.insert("C");
    let rule = serialized.get_rewriter(env, &vars).unwrap();
    rewriters.insert("re", rule).unwrap();
    let grep = TypeScript::Tsx.ast_grep("[1, 2]");
    let root = grep.root();
    let mut nm = root.find("[$A, $C]").expect("should find");
    let env = nm.get_env_mut();
    let enclosing = env.clone();
    let mut ctx = Ctx {
      lang: &TypeScript::Tsx,
      transforms: &Default::default(),
      env,
      rewriters,
      enclosing_env: &enclosing,
    };
    let ret = rewrite.compute(&mut ctx);
    assert_eq!(ret, Some("1 == 2".into()));
  }
}
