mod rewrite;
mod string_case;
mod transformation;

use crate::DeserializeEnv;
use crate::GlobalRules;

use ast_grep_core::meta_var::MetaVarEnv;
use ast_grep_core::{Doc, Language};

use std::collections::HashMap;
use thiserror::Error;

use transformation::Transformation as Trans;
pub type Transformation = Trans<String>;

#[derive(Debug, Error)]
pub enum TransformError {
  #[error("`transform` has a cyclic dependency.")]
  Cyclic,
  #[error("Transform var `{0}` has already defined.")]
  AlreadyDefined(String),
}

pub struct Transform {
  transforms: Vec<(String, Transformation)>,
}

impl Transform {
  pub fn deserialize<L: Language>(
    map: &HashMap<String, Transformation>,
    env: &DeserializeEnv<L>,
  ) -> Result<Self, TransformError> {
    let orders = env
      .get_transform_order(map)
      .map_err(|_| TransformError::Cyclic)?;
    let transforms = orders
      .into_iter()
      .map(|key| (key.to_string(), map[key].clone()))
      .collect();
    Ok(Self { transforms })
  }

  pub fn apply_transform<'c, D: Doc>(
    &self,
    lang: &D::Lang,
    env: &mut MetaVarEnv<'c, D>,
    rewriters: GlobalRules<D::Lang>,
    enclosing_env: &MetaVarEnv<'c, D>,
  ) {
    let mut ctx = Ctx {
      lang,
      env,
      rewriters,
      enclosing_env,
    };
    for (key, tr) in &self.transforms {
      tr.insert(key, &mut ctx);
    }
  }

  pub(crate) fn keys(&self) -> impl Iterator<Item = &String> {
    self.transforms.iter().map(|t| &t.0)
  }

  pub(crate) fn values(&self) -> impl Iterator<Item = &Transformation> {
    self.transforms.iter().map(|t| &t.1)
  }
}

// two lifetime to represent env root lifetime and lang/trans lifetime
struct Ctx<'b, 'c, D: Doc> {
  lang: &'b D::Lang,
  rewriters: GlobalRules<D::Lang>,
  env: &'b mut MetaVarEnv<'c, D>,
  enclosing_env: &'b MetaVarEnv<'c, D>,
}

#[cfg(test)]
mod test {
  use super::*;
  use crate::from_str;
  use crate::test::TypeScript;

  #[test]
  fn test_cyclic_transform() {
    let mut trans = HashMap::new();
    let trans_a = from_str("substring: {source: $B}").unwrap();
    trans.insert("A".into(), trans_a);
    let trans_b = from_str("substring: {source: $A}").unwrap();
    trans.insert("B".into(), trans_b);
    let env = DeserializeEnv::new(TypeScript::Tsx);
    let ret = Transform::deserialize(&trans, &env);
    assert!(matches!(ret, Err(TransformError::Cyclic)));
  }

  #[test]
  fn test_transform_use_matched() {
    let mut trans = HashMap::new();
    let trans_a = from_str("substring: {source: $C}").unwrap();
    trans.insert("A".into(), trans_a);
    let trans_b = from_str("substring: {source: $A}").unwrap();
    trans.insert("B".into(), trans_b);
    let env = DeserializeEnv::new(TypeScript::Tsx);
    let ret = Transform::deserialize(&trans, &env);
    assert!(ret.is_ok());
  }
}
