use ast_grep_core::meta_var::{MetaVarEnv, MetaVariable};
use ast_grep_core::{Language, StrDoc};

use regex::Regex;
use serde::{Deserialize, Serialize};

use std::collections::HashMap;

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Substring {
  source: String,
  start_char: Option<i32>,
  end_char: Option<i32>,
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Replace {
  source: String,
  replace: String,
  by: String,
}
impl Substring {
  fn compute<L: Language>(&self, lang: &L, env: &mut MetaVarEnv<StrDoc<L>>) -> Option<String> {
    let source = lang.pre_process_pattern(&self.source);
    let node = match lang.extract_meta_var(&source)? {
      MetaVariable::Named(n, _) => env.get_match(&n)?,
      _ => return None,
    };
    let text = node.text();
    let chars: Vec<_> = text.chars().collect();
    let len = chars.len() as i32;
    let start = resolve_char(&self.start_char, 0, len);
    let end = resolve_char(&self.end_char, len, len);
    if start > end || start >= len as usize || end > len as usize {
      return Some(String::new());
    }
    Some(chars[start..end].iter().collect())
  }
}

/// resolve relative negative char index to absolute index
/// e.g. -1 => len - 1, n > len => n
fn resolve_char(opt: &Option<i32>, dft: i32, len: i32) -> usize {
  let c = *opt.as_ref().unwrap_or(&dft);
  if c >= len {
    len as usize
  } else if c >= 0 {
    c as usize
  } else if len + c < 0 {
    0
  } else {
    debug_assert!(c < 0);
    (len + c) as usize
  }
}

impl Replace {
  fn compute<L: Language>(&self, lang: &L, env: &mut MetaVarEnv<StrDoc<L>>) -> Option<String> {
    let source = lang.pre_process_pattern(&self.source);
    let node = match lang.extract_meta_var(&source)? {
      MetaVariable::Named(n, _) => env.get_match(&n)?,
      _ => return None,
    };
    let text = node.text();
    let re = Regex::new(&self.replace).unwrap();
    Some(re.replace_all(&text, &self.by).into_owned())
  }
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub enum Transformation {
  Substring(Substring),
  Replace(Replace),
}

impl Transformation {
  fn insert<L: Language>(&self, key: &str, lang: &L, env: &mut MetaVarEnv<StrDoc<L>>) {
    // avoid cyclic
    env.insert_transformation(key.to_string(), vec![]);
    let Some(s) = self.compute(lang, env) else {
      return;
    };
    env.insert_transformation(key.to_string(), s.into_bytes());
  }
  fn compute<L: Language>(&self, lang: &L, env: &mut MetaVarEnv<StrDoc<L>>) -> Option<String> {
    use Transformation as T;
    match self {
      T::Replace(r) => r.compute(lang, env),
      T::Substring(s) => s.compute(lang, env),
    }
  }
}

pub fn apply_env_transform<L: Language>(
  transforms: &HashMap<String, Transformation>,
  lang: &L,
  env: &mut MetaVarEnv<StrDoc<L>>,
) {
  for (key, tr) in transforms {
    tr.insert(key, lang, env);
  }
}

#[cfg(test)]
mod test {
  use super::*;
  use crate::test::TypeScript;
  use serde_yaml::with::singleton_map_recursive;

  type R = std::result::Result<(), ()>;

  fn get_transformed(src: &str, pat: &str, trans: &Transformation) -> Option<String> {
    let grep = TypeScript::Tsx.ast_grep(src);
    let root = grep.root();
    let mut nm = root.find(pat).expect("should find");
    trans.compute(&TypeScript::Tsx, nm.get_env_mut())
  }

  fn parse(trans: &str) -> Result<Transformation, ()> {
    let deserializer = serde_yaml::Deserializer::from_str(trans);
    singleton_map_recursive::deserialize(deserializer).map_err(|_| ())
  }

  #[test]
  fn test_simple_replace() -> R {
    let trans = parse(
      r#"
      substring:
        source: "$A"
        startChar: 1
        endChar: -1
    "#,
    )?;
    let actual = get_transformed("let a = 123", "let a= $A", &trans).ok_or(())?;
    assert_eq!(actual, "2");
    Ok(())
  }

  #[test]
  fn test_no_end_char() -> R {
    let trans = parse(
      r#"
      substring:
        source: "$A"
        startChar: 1
    "#,
    )?;
    let actual = get_transformed("let a = 123", "let a= $A", &trans).ok_or(())?;
    assert_eq!(actual, "23");
    Ok(())
  }
  #[test]
  fn test_no_start_char() -> R {
    let trans = parse(
      r#"
      substring:
        source: "$A"
        endChar: -1
    "#,
    )?;
    let actual = get_transformed("let a = 123", "let a= $A", &trans).ok_or(())?;
    assert_eq!(actual, "12");
    Ok(())
  }

  #[test]
  fn test_replace() -> R {
    let trans = parse(
      r#"
      replace:
        source: "$A"
        replace: \d
        by: "b"
    "#,
    )?;
    let actual = get_transformed("let a = 123", "let a= $A", &trans).ok_or(())?;
    assert_eq!(actual, "bbb");
    Ok(())
  }

  #[test]
  fn test_wrong_rule() {
    let parsed = parse(
      r#"
      replace:
        source: "$A"
    "#,
    );
    assert!(parsed.is_err());
  }

  fn transform_env(trans: HashMap<String, Transformation>) -> HashMap<String, String> {
    let grep = TypeScript::Tsx.ast_grep("let a = 123");
    let root = grep.root();
    let mut nm = root.find("let a = $A").expect("should find");
    apply_env_transform(&trans, &TypeScript::Tsx, nm.get_env_mut());
    let env = nm.get_env();
    trans
      .keys()
      .map(|k| {
        (
          k.into(),
          String::from_utf8(env.get_transformed(k).unwrap().to_vec()).unwrap(),
        )
      })
      .collect()
  }

  #[test]
  fn test_insert_env() -> R {
    let tr1 = parse(
      r#"
      replace:
        source: "$A"
        replace: \d
        by: "b"
    "#,
    )?;
    let tr2 = parse(
      r#"
      substring:
        source: "$A"
        startChar: 1
        endChar: -1
    "#,
    )?;
    let mut map = HashMap::new();
    map.insert("TR1".into(), tr1);
    map.insert("TR2".into(), tr2);
    let env = transform_env(map);
    assert_eq!(env.get("TR1").expect("should get"), "bbb");
    assert_eq!(env.get("TR2").expect("should get"), "2");
    Ok(())
  }

  #[test]
  #[ignore]
  fn test_dependent_trans() -> R {
    let tr1 = parse(
      r#"
      replace:
        source: "$A"
        replace: \d
        by: "b"
    "#,
    )?;
    let tr2 = parse(
      r#"
      substring:
        source: "$TR1"
        startChar: 1
        endChar: -1
    "#,
    )?;
    let mut map = HashMap::new();
    map.insert("TR1".into(), tr1);
    map.insert("TR2".into(), tr2);
    let env = transform_env(map);
    assert_eq!(env.get("TR2").expect("should get"), "b");
    Ok(())
  }
}
