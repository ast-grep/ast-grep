use std::str::FromStr;

use ast_grep_core::language::Language;
use ast_grep_core::matcher::{Pattern, PatternBuilder, PatternError};
use serde::{de, Deserialize, Deserializer};
use std::borrow::Cow;
use std::sync::Mutex;
use wasm_bindgen::prelude::*;

use crate::doc::WasmDoc;
use crate::ts_types as ts;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WasmLang {
  JavaScript,
  TypeScript,
  Tsx,
  Bash,
  C,
  CSharp,
  Css,
  Cpp,
  Elixir,
  Go,
  Haskell,
  Html,
  Java,
  Json,
  Kotlin,
  Lua,
  Nix,
  Php,
  Python,
  Ruby,
  Rust,
  Scala,
  Swift,
  Yaml,
}

use WasmLang::*;

#[derive(Debug)]
pub struct NotSupport(String);

impl std::error::Error for NotSupport {}

impl std::fmt::Display for NotSupport {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(f, "Language {} is not supported.", self.0)
  }
}

impl FromStr for WasmLang {
  type Err = NotSupport;
  fn from_str(s: &str) -> Result<Self, Self::Err> {
    Ok(match s {
      "javascript" => JavaScript,
      "typescript" => TypeScript,
      "tsx" => Tsx,
      "bash" => Bash,
      "c" => C,
      "csharp" => CSharp,
      "css" => Css,
      "cpp" => Cpp,
      "elixir" => Elixir,
      "go" => Go,
      "html" => Html,
      "haskell" => Haskell,
      "java" => Java,
      "json" => Json,
      "lua" => Lua,
      "kotlin" => Kotlin,
      "nix" => Nix,
      "php" => Php,
      "python" => Python,
      "ruby" => Ruby,
      "rust" => Rust,
      "scala" => Scala,
      "swift" => Swift,
      "yaml" => Yaml,
      _ => return Err(NotSupport(s.to_string())),
    })
  }
}

impl<'de> Deserialize<'de> for WasmLang {
  fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
  where
    D: Deserializer<'de>,
  {
    let s = String::deserialize(deserializer)?;
    FromStr::from_str(&s).map_err(de::Error::custom)
  }
}

#[derive(Clone)]
struct TsParser(ts::Parser);

unsafe impl Send for TsParser {}
unsafe impl Sync for TsParser {}

/// Stores all loaded parsers. Multiple languages can be registered simultaneously.
static PARSERS: Mutex<Vec<(WasmLang, TsParser)>> = Mutex::new(Vec::new());

impl WasmLang {
  /// Register a language parser. Can be called multiple times for different languages.
  pub async fn register(lang: &str, parser_path: &str) -> Result<(), JsError> {
    let lang = WasmLang::from_str(lang)?;
    let parser = create_parser(parser_path).await?;
    let mut parsers = PARSERS.lock().expect_throw("set parser error");
    if let Some(entry) = parsers.iter_mut().find(|(l, _)| *l == lang) {
      entry.1 = parser;
    } else {
      parsers.push((lang, parser));
    }
    Ok(())
  }

  pub(crate) fn get_parser(&self) -> Result<ts::Parser, SgWasmError> {
    let parsers = PARSERS.lock().expect_throw("get parser error");
    parsers
      .iter()
      .find(|(l, _)| l == self)
      .map(|(_, p)| p.0.clone())
      .ok_or(SgWasmError::LanguageNotLoaded(*self))
  }

  pub(crate) fn get_ts_language(&self) -> ts::Language {
    self
      .get_parser()
      .expect_throw("language is not loaded, call setupParser first")
      .language()
      .expect_throw("parser has no language set")
  }
}

async fn create_parser(parser_path: &str) -> Result<TsParser, SgWasmError> {
  let parser = ts::Parser::new()?;
  let lang = get_lang(parser_path).await?;
  parser.set_language(Some(&lang))?;
  Ok(TsParser(parser))
}

#[cfg(target_arch = "wasm32")]
async fn get_lang(parser_path: &str) -> Result<ts::Language, SgWasmError> {
  let lang = ts::Language::load_path(parser_path).await?;
  Ok(lang)
}

#[cfg(not(target_arch = "wasm32"))]
async fn get_lang(_path: &str) -> Result<ts::Language, SgWasmError> {
  unreachable!()
}

impl Language for WasmLang {
  fn expando_char(&self) -> char {
    use WasmLang as W;
    match self {
      W::Bash => '$',
      W::C => 'µ',
      W::Cpp => 'µ',
      W::CSharp => 'µ',
      W::Css => '_',
      W::Elixir => 'µ',
      W::Go => 'µ',
      W::Html => 'z',
      W::Java => '$',
      W::JavaScript => '$',
      W::Json => '$',
      W::Haskell => 'µ',
      W::Kotlin => 'µ',
      W::Lua => '$',
      W::Nix => '_',
      W::Php => 'µ',
      W::Python => 'µ',
      W::Ruby => 'µ',
      W::Rust => 'µ',
      W::Scala => '$',
      W::Swift => 'µ',
      W::TypeScript => '$',
      W::Tsx => '$',
      W::Yaml => '$',
    }
  }

  fn build_pattern(&self, builder: &PatternBuilder) -> Result<Pattern, PatternError> {
    builder.build(|src| {
      let src = src.to_string();
      WasmDoc::try_new(src, *self).map_err(|e| e.to_string())
    })
  }

  fn pre_process_pattern<'q>(&self, query: &'q str) -> Cow<'q, str> {
    pre_process_pattern(self.expando_char(), query)
  }

  fn kind_to_id(&self, kind: &str) -> u16 {
    let lang = self.get_ts_language();
    lang.id_for_node_kind(kind, true)
  }

  fn field_to_id(&self, field: &str) -> Option<u16> {
    let lang = self.get_ts_language();
    lang.field_id_for_name(field)
  }
}

fn pre_process_pattern(expando: char, query: &str) -> Cow<'_, str> {
  let mut ret = Vec::with_capacity(query.len());
  let mut dollar_count = 0;
  for c in query.chars() {
    if c == '$' {
      dollar_count += 1;
      continue;
    }
    let need_replace = matches!(c, 'A'..='Z' | '_') || dollar_count == 3;
    let sigil = if need_replace { expando } else { '$' };
    ret.extend(std::iter::repeat(sigil).take(dollar_count));
    dollar_count = 0;
    ret.push(c);
  }
  let sigil = if dollar_count == 3 { expando } else { '$' };
  ret.extend(std::iter::repeat(sigil).take(dollar_count));
  Cow::Owned(ret.into_iter().collect())
}

// Error types

#[derive(Clone, Debug)]
pub enum SgWasmError {
  ParserError(ts::ParserError),
  LanguageError(ts::LanguageError),
  LanguageNotLoaded(WasmLang),
  FailedToParse,
}

impl std::fmt::Display for SgWasmError {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      SgWasmError::ParserError(err) => write!(f, "Parser error: {}", err.message()),
      SgWasmError::LanguageError(err) => write!(f, "Language error: {:?}", err),
      SgWasmError::LanguageNotLoaded(lang) => {
        write!(
          f,
          "Language {:?} is not loaded. Call setupParser first.",
          lang
        )
      }
      SgWasmError::FailedToParse => write!(f, "Failed to parse"),
    }
  }
}

impl std::error::Error for SgWasmError {}

impl From<ts::ParserError> for SgWasmError {
  fn from(err: ts::ParserError) -> Self {
    SgWasmError::ParserError(err)
  }
}

impl From<ts::LanguageError> for SgWasmError {
  fn from(err: ts::LanguageError) -> Self {
    SgWasmError::LanguageError(err)
  }
}
