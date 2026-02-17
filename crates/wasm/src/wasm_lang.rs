use std::collections::HashMap;
use std::fmt;
use std::str::FromStr;

use ast_grep_core::language::Language;
use ast_grep_core::matcher::{Pattern, PatternBuilder, PatternError};
use serde::{de, Deserialize, Deserializer, Serialize};
use std::borrow::Cow;
use std::sync::Mutex;
use wasm_bindgen::prelude::*;

use crate::doc::WasmDoc;
use crate::ts_types as ts;

type LangIndex = u32;

/// Represents a dynamically registered language in WASM.
/// Languages are not predefined — they must be registered at runtime via `register`.
#[derive(Copy, Clone, PartialEq, Eq, Hash)]
pub struct WasmLang {
  index: LangIndex,
  // inline expando char since it is used frequently
  expando: char,
}

impl fmt::Debug for WasmLang {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    let langs = LANGS.lock().expect("debug lock");
    if let Some(inner) = langs.get(self.index as usize) {
      write!(f, "WasmLang({})", inner.name)
    } else {
      write!(f, "WasmLang(#{})", self.index)
    }
  }
}

#[derive(Debug)]
pub struct NotSupport(String);

impl std::error::Error for NotSupport {}

impl fmt::Display for NotSupport {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(
      f,
      "Language `{}` is not registered. Call registerDynamicLanguage first.",
      self.0
    )
  }
}

impl FromStr for WasmLang {
  type Err = NotSupport;
  fn from_str(s: &str) -> Result<Self, Self::Err> {
    let langs = LANGS.lock().expect_throw("from_str lock error");
    for (i, inner) in langs.iter().enumerate() {
      if inner.name == s {
        return Ok(WasmLang {
          index: i as LangIndex,
          expando: inner.expando_char,
        });
      }
    }
    Err(NotSupport(s.to_string()))
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

impl Serialize for WasmLang {
  fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
  where
    S: serde::Serializer,
  {
    let langs = LANGS.lock().expect("serialize lock");
    if let Some(inner) = langs.get(self.index as usize) {
      serializer.serialize_str(&inner.name)
    } else {
      serializer.serialize_str(&format!("unknown#{}", self.index))
    }
  }
}

#[derive(Clone)]
struct TsParser(ts::Parser);

unsafe impl Send for TsParser {}
unsafe impl Sync for TsParser {}

struct Inner {
  name: String,
  parser: TsParser,
  expando_char: char,
}

/// Registration info for a custom WASM language, mirroring napi/pyo3's CustomLang.
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WasmLangInfo {
  pub library_path: String,
  pub expando_char: Option<char>,
}

/// Stores all registered languages.
static LANGS: Mutex<Vec<Inner>> = Mutex::new(Vec::new());

impl WasmLang {
  /// Register languages from a HashMap of name -> WasmLangInfo.
  /// Can be called multiple times; existing languages are updated.
  pub async fn register(langs: HashMap<String, WasmLangInfo>) -> Result<(), JsError> {
    for (name, custom) in langs {
      let parser = create_parser(&custom.library_path).await?;
      let expando = custom.expando_char.unwrap_or('$');
      let mut registered = LANGS.lock().expect_throw("register lock error");
      if let Some(entry) = registered.iter_mut().find(|inner| inner.name == name) {
        entry.parser = parser;
        entry.expando_char = expando;
      } else {
        registered.push(Inner {
          name,
          parser,
          expando_char: expando,
        });
      }
    }
    Ok(())
  }

  pub(crate) fn get_parser(&self) -> Result<ts::Parser, SgWasmError> {
    let langs = LANGS.lock().expect_throw("get parser error");
    match langs.get(self.index as usize) {
      Some(inner) => Ok(inner.parser.0.clone()),
      None => {
        let name = format!("lang#{}", self.index);
        Err(SgWasmError::LanguageNotLoaded(name))
      }
    }
  }

  pub(crate) fn get_ts_language(&self) -> ts::Language {
    self
      .get_parser()
      .expect_throw("language is not loaded, call registerDynamicLanguage first")
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

async fn get_lang(parser_path: &str) -> Result<ts::Language, SgWasmError> {
  let lang = ts::Language::load_path(parser_path).await?;
  Ok(lang)
}

impl Language for WasmLang {
  fn expando_char(&self) -> char {
    self.expando
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
  LanguageNotLoaded(String),
  FailedToParse,
}

impl fmt::Display for SgWasmError {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      SgWasmError::ParserError(err) => write!(f, "Parser error: {}", err.message()),
      SgWasmError::LanguageError(err) => write!(f, "Language error: {:?}", err),
      SgWasmError::LanguageNotLoaded(name) => {
        write!(
          f,
          "Language `{}` is not loaded. Call registerDynamicLanguage first.",
          name
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
