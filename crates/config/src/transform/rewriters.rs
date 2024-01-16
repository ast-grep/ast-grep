use super::Ctx;

use ast_grep_core::Doc;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Rewriters {
  rewrites: Vec<String>,
  join_by: Option<String>,
}

impl Rewriters {
  pub fn compute<D: Doc>(&self, ctx: &mut Ctx<D>) -> Option<String> {
    // TODO
    None
  }
}
