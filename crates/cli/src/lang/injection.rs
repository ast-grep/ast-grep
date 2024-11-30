use super::SgLang;
use crate::utils::ErrorContext as EC;
use ast_grep_config::{DeserializeEnv, RuleCore, SerializableRuleCore};
use ast_grep_core::{language::TSRange, Doc, Language, Node, StrDoc};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use std::collections::{HashMap, HashSet};
use std::ptr::{addr_of, addr_of_mut};
use std::str::FromStr;

// NB, you should not use SgLang in the (de_serialize interface
// since Injected is used before lang registration in sgconfig.yml
#[derive(Serialize, Deserialize, Clone)]
#[serde(untagged)]
pub enum Injected {
  Static(String),
  Dynamic(Vec<String>),
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SerializableInjection {
  #[serde(flatten)]
  core: SerializableRuleCore,
  /// The host language, e.g. html, contains other languages
  host_language: String,
  /// Injected language according to the rule
  /// It accepts either a string like js for single static language.
  /// or an array of string like [js, ts] for dynamic language detection.
  injected: Injected,
}

struct Injection {
  host: SgLang,
  rules: Vec<(RuleCore<SgLang>, Option<String>)>,
  injectable: HashSet<String>,
}

impl Injection {
  fn new(lang: SgLang) -> Self {
    Self {
      host: lang,
      rules: vec![],
      injectable: Default::default(),
    }
  }
}

pub unsafe fn register_injetables(injections: Vec<SerializableInjection>) -> Result<()> {
  let mut injectable = HashMap::new();
  for injection in injections {
    register_injetable(injection, &mut injectable)?;
  }
  merge_default_injecatable(&mut injectable);
  *addr_of_mut!(LANG_INJECTIONS) = injectable.into_values().collect();
  let injects = unsafe { &*addr_of!(LANG_INJECTIONS) as &'static Vec<Injection> };
  *addr_of_mut!(INJECTABLE_LANGS) = injects
    .iter()
    .map(|inj| {
      (
        inj.host,
        inj.injectable.iter().map(|s| s.as_str()).collect(),
      )
    })
    .collect();
  Ok(())
}

fn merge_default_injecatable(ret: &mut HashMap<SgLang, Injection>) {
  for (lang, injection) in ret {
    let langs = match lang {
      SgLang::Builtin(b) => b.injectable_languages(),
      SgLang::Custom(c) => c.injectable_languages(),
    };
    let Some(langs) = langs else {
      continue;
    };
    injection
      .injectable
      .extend(langs.iter().map(|s| s.to_string()));
  }
}

fn register_injetable(
  injection: SerializableInjection,
  injectable: &mut HashMap<SgLang, Injection>,
) -> Result<()> {
  let lang = SgLang::from_str(&injection.host_language)?;
  let env = DeserializeEnv::new(lang);
  let rule = injection.core.get_matcher(env).context(EC::LangInjection)?;
  let default_lang = match &injection.injected {
    Injected::Static(s) => Some(s.clone()),
    Injected::Dynamic(_) => None,
  };
  let entry = injectable
    .entry(lang)
    .or_insert_with(|| Injection::new(lang));
  match injection.injected {
    Injected::Static(s) => {
      entry.injectable.insert(s);
    }
    Injected::Dynamic(v) => entry.injectable.extend(v),
  }
  entry.rules.push((rule, default_lang));
  Ok(())
}

static mut LANG_INJECTIONS: Vec<Injection> = vec![];
static mut INJECTABLE_LANGS: Vec<(SgLang, Vec<&'static str>)> = vec![];

pub fn injectable_languages(lang: SgLang) -> Option<&'static [&'static str]> {
  // NB: custom injection and builtin injections are resolved in INJECTABLE_LANGS
  let injections =
    unsafe { &*addr_of!(INJECTABLE_LANGS) as &'static Vec<(SgLang, Vec<&'static str>)> };
  let Some(injection) = injections.iter().find(|i| i.0 == lang) else {
    return match lang {
      SgLang::Builtin(b) => b.injectable_languages(),
      SgLang::Custom(c) => c.injectable_languages(),
    };
  };
  Some(&injection.1)
}

pub fn extract_injections<D: Doc>(root: Node<D>) -> HashMap<String, Vec<TSRange>> {
  // NB Only works in the CLI crate because we only has Node<SgLang>
  let root: Node<StrDoc<SgLang>> = unsafe { std::mem::transmute(root) };
  let mut ret = match root.lang() {
    SgLang::Custom(c) => c.extract_injections(root.clone()),
    SgLang::Builtin(b) => b.extract_injections(root.clone()),
  };
  let injections = unsafe { &*addr_of!(LANG_INJECTIONS) };
  extract_custom_inject(injections, root, &mut ret);
  ret
}

fn extract_custom_inject(
  injections: &[Injection],
  root: Node<StrDoc<SgLang>>,
  ret: &mut HashMap<String, Vec<TSRange>>,
) {
  let Some(rules) = injections.iter().find(|n| n.host == *root.lang()) else {
    return;
  };
  for (rule, default_lang) in &rules.rules {
    for m in root.find_all(rule) {
      let env = m.get_env();
      let Some(region) = env.get_match("CONTENT") else {
        continue;
      };
      let Some(lang) = env
        .get_match("LANG")
        .map(|n| n.text().to_string())
        .or_else(|| default_lang.clone())
      else {
        continue;
      };
      let range = node_to_range(region);
      ret.entry(lang).or_default().push(range);
    }
  }
}

fn node_to_range<D: Doc>(node: &Node<D>) -> TSRange {
  let r = node.range();
  let start = node.start_pos();
  let sp = start.ts_point();
  let end = node.end_pos();
  let ep = end.ts_point();
  TSRange::new(r.start as u32, r.end as u32, &sp, &ep)
}

#[cfg(test)]
mod test {
  use super::*;
  use ast_grep_config::from_str;
  use ast_grep_language::SupportLang;
  const DYNAMIC: &str = "
hostLanguage: js
rule:
  pattern: styled.$LANG`$CONTENT`
injected: [css]";
  const STATIC: &str = "
hostLanguage: js
rule:
  pattern: styled`$CONTENT`
injected: css";
  #[test]
  fn test_deserialize() {
    let inj: SerializableInjection = from_str(STATIC).expect("should ok");
    assert!(matches!(inj.injected, Injected::Static(_)));
    let inj: SerializableInjection = from_str(DYNAMIC).expect("should ok");
    assert!(matches!(inj.injected, Injected::Dynamic(_)));
  }

  const BAD: &str = "
hostLanguage: HTML
rule:
  kind: not_exist
injected: [js, ts, tsx]";

  #[test]
  fn test_bad_inject() {
    let mut map = HashMap::new();
    let inj: SerializableInjection = from_str(BAD).expect("should ok");
    let ret = register_injetable(inj, &mut map);
    assert!(ret.is_err());
    let ec = ret.unwrap_err().downcast::<EC>().expect("should ok");
    assert!(matches!(ec, EC::LangInjection));
  }

  #[test]
  fn test_good_injection() {
    let mut map = HashMap::new();
    let inj: SerializableInjection = from_str(STATIC).expect("should ok");
    let ret = register_injetable(inj, &mut map);
    assert!(ret.is_ok());
    let inj: SerializableInjection = from_str(DYNAMIC).expect("should ok");
    let ret = register_injetable(inj, &mut map);
    assert!(ret.is_ok());
    assert_eq!(map.len(), 1);
    let injections: Vec<_> = map.into_values().collect();
    let mut ret = HashMap::new();
    let sg =
      SgLang::from(SupportLang::JavaScript).ast_grep("const a = styled`.btn { margin: 0; }`");
    let root = sg.root();
    extract_custom_inject(&injections, root, &mut ret);
    assert_eq!(ret.len(), 1);
    assert_eq!(ret["css"].len(), 1);
    assert!(!ret.contains_key("js"));
    ret.clear();
    let sg =
      SgLang::from(SupportLang::JavaScript).ast_grep("const a = styled.css`.btn { margin: 0; }`");
    let root = sg.root();
    extract_custom_inject(&injections, root, &mut ret);
    assert_eq!(ret.len(), 1);
    assert_eq!(ret["css"].len(), 1);
    assert!(!ret.contains_key("js"));
  }
}
