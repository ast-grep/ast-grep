use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Represents a position in a document
#[derive(Serialize, Deserialize, Clone, JsonSchema)]
pub struct SerializablePosition {
  pub row: usize,
  pub column: usize,
}

/// Represents a range rule with a start and end position
#[derive(Serialize, Deserialize, Clone, JsonSchema)]
pub struct SerializableRange {
  pub start: SerializablePosition,
  pub end: SerializablePosition,
}
