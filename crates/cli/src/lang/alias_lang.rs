use super::SupportLang;
use ignore::types::{Types, TypesBuilder};
use serde::{Deserialize, Serialize};

use std::collections::HashMap;
use std::ops::Deref;
use std::path::Path;
use std::str::FromStr;

// both use vec since lang will be small
static mut ALIAS_LANG: Vec<Inner> = vec![];
static mut LANG_INDEX: Vec<(String, u32)> = vec![];

type LangIndex = u32;

#[derive(Copy, Clone, PartialEq, Eq)]
pub struct AliasLang {
  index: LangIndex,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AliasRegistration {
  pub alias_of: SupportLang,
  pub extensions: Vec<String>,
}

struct Inner {
  lang: SupportLang,
  name: String,
}

impl AliasLang {
  pub fn all_langs() -> Vec<Self> {
    (0..Self::langs().len())
      .map(|index| AliasLang {
        index: index as LangIndex,
      })
      .collect()
  }
  pub fn file_types(&self) -> Types {
    let mut builder = TypesBuilder::new();
    let inner = self.inner();
    let mapping = unsafe { &LANG_INDEX };
    for (ext, i) in mapping.iter() {
      if *i == self.index {
        builder
          .add(&inner.name, &format!("*.{ext}"))
          .expect("file pattern must compile");
      }
    }
    builder.select(&inner.name);
    builder.build().expect("file type must be valid")
  }
}

impl AliasLang {
  /// # Safety
  /// the register function should be called exactly once before use.
  /// It relies on a global mut static variable to be initialized.
  pub unsafe fn register(regs: HashMap<String, AliasRegistration>) {
    debug_assert!(Self::langs().is_empty());
    let mut langs = vec![];
    let mut mapping = vec![];
    for (lang_name, reg) in regs {
      Self::register_one(lang_name, reg, &mut langs, &mut mapping);
    }
    _ = std::mem::replace(&mut ALIAS_LANG, langs);
    _ = std::mem::replace(&mut LANG_INDEX, mapping);
  }

  pub fn name(&self) -> &str {
    &self.inner().name
  }

  pub fn from_path<P: AsRef<Path>>(path: P) -> Option<Self> {
    let ext = path.as_ref().extension()?.to_str()?;
    let mapping = unsafe { &LANG_INDEX };
    mapping.iter().find_map(|(p, idx)| {
      if p == ext {
        let index = *idx;
        Some(Self { index })
      } else {
        None
      }
    })
  }

  fn register_one(
    lang_name: String,
    reg: AliasRegistration,
    langs: &mut Vec<Inner>,
    mapping: &mut Vec<(String, LangIndex)>,
  ) {
    let inner = Inner {
      name: lang_name,
      lang: reg.alias_of,
    };
    langs.push(inner);
    let idx = langs.len() as LangIndex - 1;
    for ext in reg.extensions {
      mapping.push((ext, idx));
    }
  }
  fn inner(&self) -> &Inner {
    let langs = Self::langs();
    &langs[self.index as usize]
  }

  fn langs() -> &'static Vec<Inner> {
    unsafe { &ALIAS_LANG }
  }
}

impl Serialize for AliasLang {
  fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
  where
    S: serde::Serializer,
  {
    let name = &self.inner().name;
    serializer.serialize_str(name)
  }
}

impl<'de> Deserialize<'de> for AliasLang {
  fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
  where
    D: serde::Deserializer<'de>,
  {
    let name = String::deserialize(deserializer)?;
    AliasLang::from_str(&name).map_err(serde::de::Error::custom)
  }
}

impl FromStr for AliasLang {
  type Err = String;
  fn from_str(name: &str) -> Result<Self, Self::Err> {
    let langs = Self::langs();
    for (i, lang) in langs.iter().enumerate() {
      if lang.name == name {
        return Ok(AliasLang {
          index: i as LangIndex,
        });
      }
    }
    Err(format!("unknow language `{name}`."))
  }
}

impl Deref for AliasLang {
  type Target = SupportLang;
  fn deref(&self) -> &Self::Target {
    &self.inner().lang
  }
}
