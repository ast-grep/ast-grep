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
  #[error("Transform has a cyclic dependency.")]
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
      transforms: &self.transforms,
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
  transforms: &'b Vec<(String, Transformation)>,
  lang: &'b D::Lang,
  rewriters: GlobalRules<D::Lang>,
  env: &'b mut MetaVarEnv<'c, D>,
  enclosing_env: &'b MetaVarEnv<'c, D>,
}
