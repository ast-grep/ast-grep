mod rewrite;
mod string_case;
mod transformation;

use crate::GlobalRules;
use ast_grep_core::meta_var::MetaVarEnv;
use ast_grep_core::Doc;

use std::collections::HashMap;
use transformation::Transformation as Trans;
pub type Transformation = Trans<String>;

// two lifetime to represent env root lifetime and lang/trans lifetime
struct Ctx<'b, 'c, D: Doc> {
  transforms: &'b HashMap<String, Transformation>,
  lang: &'b D::Lang,
  rewriters: GlobalRules<D::Lang>,
  env: &'b mut MetaVarEnv<'c, D>,
  enclosing_env: &'b MetaVarEnv<'c, D>,
}

pub fn apply_env_transform<'c, D: Doc>(
  transforms: &HashMap<String, Transformation>,
  lang: &D::Lang,
  env: &mut MetaVarEnv<'c, D>,
  rewriters: GlobalRules<D::Lang>,
  enclosing_env: &MetaVarEnv<'c, D>,
) {
  let mut ctx = Ctx {
    transforms,
    lang,
    env,
    rewriters,
    enclosing_env,
  };
  for (key, tr) in transforms {
    tr.insert(key, &mut ctx);
  }
}
