use super::SgLang;
use ast_grep_config::{DeserializeEnv, RuleCore, SerializableRuleCore};
use ast_grep_core::{
  language::{TSPoint, TSRange},
  Doc, Language, Node, StrDoc,
};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::ptr::{addr_of, addr_of_mut};

#[derive(Serialize, Deserialize, Clone)]
#[serde(untagged)]
pub enum Injected {
  Static(SgLang),
  Dynamic(Vec<SgLang>),
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SerializableInjection {
  #[serde(flatten)]
  core: SerializableRuleCore,
  /// The host language, e.g. html, contains other languages
  host_language: SgLang,
  /// Injected language according to the rule
  /// It accepts either a string like js for single static language.
  /// or an array of string like [js, ts] for dynamic language detection.
  injected: Injected,
}

struct Injection {
  host: SgLang,
  rules: Vec<(RuleCore<SgLang>, Option<String>)>,
  injectable: HashSet<SgLang>,
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

// fn injectable_languages(&self) -> Option<&'static [&'static str]>
// fn extract_injections<D: Doc>(&self, root: Node<D>) -> HashMap<String, Vec<TSRange>> {
// lang => embeddedlangs
// lang => Vec<rules>
pub unsafe fn register_injetables(injections: Vec<SerializableInjection>) {
  let mut injectable = HashMap::new();
  for injection in injections {
    register_injetable(injection, &mut injectable);
  }
  *addr_of_mut!(LANG_INJECTIONS) = injectable.into_values().collect();
}

fn register_injetable(
  injection: SerializableInjection,
  injectable: &mut HashMap<SgLang, Injection>,
) {
  let env = DeserializeEnv::new(injection.host_language);
  let rule = injection.core.get_matcher(env).expect("TODO");
  let default_lang = match injection.injected {
    Injected::Static(s) => Some(format!("{s}")),
    Injected::Dynamic(_) => None,
  };
  let entry = injectable
    .entry(injection.host_language)
    .or_insert_with(|| Injection::new(injection.host_language));
  match injection.injected {
    Injected::Static(s) => {
      entry.injectable.insert(s);
    }
    Injected::Dynamic(v) => entry.injectable.extend(v),
  }
  entry.rules.push((rule, default_lang));
}

static mut LANG_INJECTIONS: Vec<Injection> = vec![];

pub fn extract_injections<D: Doc>(root: Node<D>) -> HashMap<String, Vec<TSRange>> {
  // NB Only works in the CLI crate because we only has Node<SgLang>
  let root: Node<StrDoc<SgLang>> = unsafe { std::mem::transmute(root) };
  let mut ret = match root.lang() {
    SgLang::Custom(c) => c.extract_injections(root.clone()),
    SgLang::Builtin(b) => b.extract_injections(root.clone()),
  };
  extract_custom_inject(root, &mut ret);
  ret
}

fn extract_custom_inject(root: Node<StrDoc<SgLang>>, ret: &mut HashMap<String, Vec<TSRange>>) {
  let injections = unsafe { &*addr_of!(LANG_INJECTIONS) };
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
  let sp = TSPoint::new(start.0 as u32, start.1 as u32);
  let end = node.end_pos();
  let ep = TSPoint::new(end.0 as u32, end.1 as u32);
  TSRange::new(r.start as u32, r.end as u32, &sp, &ep)
}

#[cfg(test)]
mod test {
  use super::*;
  use ast_grep_config::from_str;
  const DYNAMIC: &str = "
hostLanguage: HTML
rule:
  pattern: <script lang=$LANG>$CONTENT</script>
injected: [js, ts, tsx]";
  const STATIC: &str = "
hostLanguage: HTML
rule:
  pattern: <script>$CONTENT</script>
injected: js";
  #[test]
  fn test_deserialize() {
    let inj: SerializableInjection = from_str(STATIC).expect("should ok");
    assert!(matches!(inj.injected, Injected::Static(_)));
    let inj: SerializableInjection = from_str(DYNAMIC).expect("should ok");
    assert!(matches!(inj.injected, Injected::Dynamic(_)));
  }
}
