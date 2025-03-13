//! This mod maintains a list of tree-sitter parsers crate.
//! When feature flag `builtin-parser` is on, this mod will import all dependent crates.
//! However, tree-sitter bs cannot be compiled by wasm-pack.
//! In this case, we can use a blank implementation by turning feature flag off.
//! And use other implementation.

#[cfg(any(feature = "builtin-parser", feature = "wasm-exhaustive-lang"))]
mod parsers_builtin;

#[cfg(feature = "napi-lang")]
mod parsers_napi;

#[cfg(feature = "wasm-lang")]
mod parsers_wasm;

#[cfg(any(
  not(feature = "builtin-parser"),
  not(feature = "napi-lang"),
  not(feature = "wasm-lang"),
  not(feature = "wasm-exhaustive-lang")
))]
mod parsers_none;

// Re-export language functions based on enabled features
// Use mutually exclusive conditions to avoid ambiguity
#[cfg(any(feature = "builtin-parser", feature = "wasm-exhaustive-lang"))]
pub use parsers_builtin::parsers_builtin::*;

#[cfg(all(
  feature = "napi-lang",
  not(feature = "builtin-parser"),
  not(feature = "wasm-exhaustive-lang")
))]
pub use parsers_napi::parsers_napi::*;

#[cfg(all(
  feature = "wasm-lang",
  not(feature = "builtin-parser"),
  not(feature = "napi-lang"),
  not(feature = "wasm-exhaustive-lang")
))]
pub use parsers_wasm::parsers_wasm::*;

#[cfg(all(
  not(feature = "builtin-parser"),
  not(feature = "napi-lang"),
  not(feature = "wasm-lang"),
  not(feature = "wasm-exhaustive-lang")
))]
pub use parsers_none::parsers_none::*;
