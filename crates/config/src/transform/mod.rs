mod rewrite;
mod string_case;

use crate::GlobalRules;
use ast_grep_core::meta_var::{MetaVarEnv, MetaVariable};
use ast_grep_core::source::Content;
use ast_grep_core::{Doc, Language};
pub use rewrite::Rewrite;

use regex::Regex;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use std::collections::HashMap;
use string_case::{Separator, StringCase};

fn get_text_from_env<D: Doc>(src: &str, ctx: &mut Ctx<D>) -> Option<String> {
  let source = ctx.lang.pre_process_pattern(src);
  let var = ctx.lang.extract_meta_var(&source)?;
  if let MetaVariable::Capture(n, _) = &var {
    if let Some(tr) = ctx.transforms.get(n) {
      if ctx.env.get_transformed(n).is_none() {
        tr.insert(n, ctx);
      }
    }
  }
  let bytes = ctx.env.get_var_bytes(&var)?;
  Some(<D::Source as Content>::encode_bytes(bytes).into_owned())
}

/// Extracts a substring from the meta variable's text content.
///
/// Both `start_char` and `end_char` support negative indexing,
/// which counts character from the end of an array, moving backwards.
#[derive(Serialize, Deserialize, Clone, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Substring {
  /// source meta variable to be transformed
  source: String,
  /// optional starting character index of the substring, defaults to 0.
  start_char: Option<i32>,
  /// optional ending character index of the substring, defaults to the end of the string.
  end_char: Option<i32>,
}

impl Substring {
  fn compute<D: Doc>(&self, ctx: &mut Ctx<D>) -> Option<String> {
    let text = get_text_from_env(&self.source, ctx)?;
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

/// Replaces a substring in the meta variable's text content with another string.
#[derive(Serialize, Deserialize, Clone, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Replace {
  /// source meta variable to be transformed
  source: String,
  /// a regex to find substring to be replaced
  replace: String,
  /// the replacement string
  by: String,
}
impl Replace {
  fn compute<D: Doc>(&self, ctx: &mut Ctx<D>) -> Option<String> {
    let text = get_text_from_env(&self.source, ctx)?;
    let re = Regex::new(&self.replace).unwrap();
    Some(re.replace_all(&text, &self.by).into_owned())
  }
}

/// Converts the source meta variable's text content to a specified case format.
#[derive(Serialize, Deserialize, Clone, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Convert {
  /// source meta variable to be transformed
  source: String,
  /// the target case format to convert the text content to
  to_case: StringCase,
  /// optional separators to specify how to separate word
  separated_by: Option<Vec<Separator>>,
}
impl Convert {
  fn compute<D: Doc>(&self, ctx: &mut Ctx<D>) -> Option<String> {
    let text = get_text_from_env(&self.source, ctx)?;
    Some(self.to_case.apply(&text, self.separated_by.as_deref()))
  }
}

/// Represents a transformation that can be applied to a matched AST node.
/// Available transformations are `substring`, `replace` and `convert`.
#[derive(Serialize, Deserialize, Clone, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum Transformation {
  Substring(Substring),
  Replace(Replace),
  Convert(Convert),
  Rewrite(Rewrite),
}

impl Transformation {
  fn insert<D: Doc>(&self, key: &str, ctx: &mut Ctx<D>) {
    // avoid cyclic
    ctx.env.insert_transformation(key, vec![]);
    let opt = self.compute(ctx);
    let bytes = if let Some(s) = opt {
      <D::Source as Content>::decode_str(&s).to_vec()
    } else {
      vec![]
    };
    ctx.env.insert_transformation(key, bytes);
  }
  fn compute<D: Doc>(&self, ctx: &mut Ctx<D>) -> Option<String> {
    use Transformation as T;
    match self {
      T::Replace(r) => r.compute(ctx),
      T::Substring(s) => s.compute(ctx),
      T::Convert(c) => c.compute(ctx),
      T::Rewrite(r) => r.compute(ctx),
    }
  }
}

// two lifetime to represent env root lifetime and lang/trans lifetime
struct Ctx<'b, 'c, D: Doc> {
  transforms: &'b HashMap<String, Transformation>,
  lang: &'b D::Lang,
  rewriters: GlobalRules<D::Lang>,
  env: &'b mut MetaVarEnv<'c, D>,
}

pub fn apply_env_transform<D: Doc>(
  transforms: &HashMap<String, Transformation>,
  lang: &D::Lang,
  env: &mut MetaVarEnv<D>,
  rewriters: GlobalRules<D::Lang>,
) {
  let mut ctx = Ctx {
    transforms,
    lang,
    env,
    rewriters,
  };
  for (key, tr) in transforms {
    tr.insert(key, &mut ctx);
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
    let mut ctx = Ctx {
      lang: &TypeScript::Tsx,
      transforms: &HashMap::new(),
      env: nm.get_env_mut(),
      rewriters: Default::default(),
    };
    trans.compute(&mut ctx)
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
    apply_env_transform(
      &trans,
      &TypeScript::Tsx,
      nm.get_env_mut(),
      Default::default(),
    );
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
    let up = parse(
      r#"
      convert:
        source: "$SUB"
        toCase: upperCase
    "#,
    )?;
    let mut map = HashMap::new();
    map.insert("REP".into(), rep);
    map.insert("SUB".into(), sub);
    map.insert("UP".into(), up);
    let env = transform_env(map);
    assert_eq!(env["REP"], "bbb");
    assert_eq!(env["SUB"], "b");
    assert_eq!(env["UP"], "B");
    Ok(())
  }

  #[test]
  fn test_uppercase_convert() -> R {
    let trans = parse(
      r#"
      convert:
        source: "$A"
        toCase: upperCase
    "#,
    )?;
    let actual = get_transformed("let a = real_quiet_now", "let a = $A", &trans).ok_or(())?;
    assert_eq!(actual, "REAL_QUIET_NOW");
    Ok(())
  }

  #[test]
  fn test_capitalize_convert() -> R {
    let trans = parse(
      r#"
      convert:
        source: "$A"
        toCase: capitalize
    "#,
    )?;
    let actual = get_transformed("let a = snugglebunny", "let a = $A", &trans).ok_or(())?;
    assert_eq!(actual, "Snugglebunny");
    Ok(())
  }

  #[test]
  fn test_lowercase_convert() -> R {
    let trans = parse(
      r#"
      convert:
        source: "$A"
        toCase: lowerCase
    "#,
    )?;
    let actual = get_transformed("let a = SCREAMS", "let a = $A", &trans).ok_or(())?;
    assert_eq!(actual, "screams");
    Ok(())
  }

  #[test]
  fn test_separation_convert() -> R {
    let trans = parse(
      r#"
      convert:
        source: "$A"
        toCase: snakeCase
        separatedBy: [underscore]
    "#,
    )?;
    let actual = get_transformed("let a = camelCase_Not", "let a = $A", &trans).ok_or(())?;
    assert_eq!(actual, "camelcase_not");
    Ok(())
  }
  // TODO: add a symbolic test for Rewrite
}
