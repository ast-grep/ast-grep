use ast_grep_core::meta_var::{MetaVarEnv, MetaVariable};
use ast_grep_core::{Language, StrDoc};

use regex::Regex;
use serde::{Deserialize, Serialize};

use std::collections::HashMap;

fn get_text_from_env<L: Language>(
  src: &str,
  ctx: &Ctx<L>,
  env: &mut MetaVarEnv<StrDoc<L>>,
) -> Option<String> {
  let source = ctx.lang.pre_process_pattern(src);
  let var = ctx.lang.extract_meta_var(&source)?;
  if let MetaVariable::Named(n, _) = &var {
    if let Some(tr) = ctx.transforms.get(n) {
      if env.get_transformed(n).is_none() {
        tr.insert(n, ctx, env);
      }
    }
  }
  let bytes = env.get_var_bytes(&var)?;
  Some(String::from_utf8_lossy(bytes).into_owned())
}

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
  fn compute<L: Language>(&self, ctx: &Ctx<L>, env: &mut MetaVarEnv<StrDoc<L>>) -> Option<String> {
    let text = get_text_from_env(&self.source, ctx, env)?;
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
  fn compute<L: Language>(&self, ctx: &Ctx<L>, env: &mut MetaVarEnv<StrDoc<L>>) -> Option<String> {
    let text = get_text_from_env(&self.source, ctx, env)?;
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
  fn insert<L: Language>(&self, key: &str, ctx: &Ctx<L>, env: &mut MetaVarEnv<StrDoc<L>>) {
    // avoid cyclic
    env.insert_transformation(key.to_string(), vec![]);
    let opt = self.compute(ctx, env);
    let bytes = if let Some(s) = opt {
      s.into_bytes()
    } else {
      vec![]
    };
    env.insert_transformation(key.to_string(), bytes);
  }
  fn compute<L: Language>(&self, ctx: &Ctx<L>, env: &mut MetaVarEnv<StrDoc<L>>) -> Option<String> {
    use Transformation as T;
    match self {
      T::Replace(r) => r.compute(ctx, env),
      T::Substring(s) => s.compute(ctx, env),
    }
  }
}

struct Ctx<'b, L: Language> {
  transforms: &'b HashMap<String, Transformation>,
  lang: &'b L,
  // env: &'b mut MetaVarEnv<'b, StrDoc<L>>,
}

pub fn apply_env_transform<L: Language>(
  transforms: &HashMap<String, Transformation>,
  lang: &L,
  env: &mut MetaVarEnv<StrDoc<L>>,
) {
  let ctx = Ctx { transforms, lang };
  for (key, tr) in transforms {
    tr.insert(key, &ctx, env);
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
    let ctx = Ctx {
      lang: &TypeScript::Tsx,
      transforms: &HashMap::new(),
    };
    trans.compute(&ctx, nm.get_env_mut())
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
    nm.get_env().clone().into()
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
    assert_eq!(env["TR1"], "bbb");
    assert_eq!(env["TR2"], "2");
    Ok(())
  }

  #[test]
  fn test_dependent_trans() -> R {
    let rep = parse(
      r#"
      replace:
        source: "$A"
        replace: \d
        by: "b"
    "#,
    )?;
    let sub = parse(
      r#"
      substring:
        source: "$REP"
        startChar: 1
        endChar: -1
    "#,
    )?;
    let mut map = HashMap::new();
    map.insert("REP".into(), rep);
    map.insert("SUB".into(), sub);
    let env = transform_env(map);
    assert_eq!(env["REP"], "bbb");
    assert_eq!(env["SUB"], "b");
    Ok(())
  }
}
