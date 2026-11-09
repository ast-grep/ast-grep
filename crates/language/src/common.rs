use serde::de::{self, Visitor};
use std::fmt;

pub(super) const fn sort_languages<const N: usize>(
  mut langs: [super::SupportLang; N],
  mut names: [&str; N],
) -> [super::SupportLang; N] {
  let mut i = 1;
  while i < N {
    let mut j = i;
    while j > 0 && name_less(names[j], names[j - 1]) {
      langs.swap(j, j - 1);
      names.swap(j, j - 1);
      j -= 1;
    }
    i += 1;
  }
  langs
}

const fn name_less(a: &str, b: &str) -> bool {
  let a = a.as_bytes();
  let b = b.as_bytes();
  let mut i = 0;
  while i < a.len() && i < b.len() {
    let left = a[i].to_ascii_lowercase();
    let right = b[i].to_ascii_lowercase();
    if left != right {
      return left < right;
    }
    i += 1;
  }
  a.len() < b.len()
}

#[allow(
  dead_code,
  reason = "Only languages with an expando character use this helper."
)]
pub(super) fn pre_process_pattern(expando: char, query: &str) -> std::borrow::Cow<'_, str> {
  let mut ret = Vec::with_capacity(query.len());
  let mut dollar_count = 0;
  for c in query.chars() {
    if c == '$' {
      dollar_count += 1;
      continue;
    }
    let need_replace = matches!(c, 'A'..='Z' | '_') // $A or $$A or $$$A
      || dollar_count == 3; // anonymous multiple
    let sigil = if need_replace { expando } else { '$' };
    ret.extend(std::iter::repeat_n(sigil, dollar_count));
    dollar_count = 0;
    ret.push(c);
  }
  // trailing anonymous multiple
  let sigil = if dollar_count == 3 { expando } else { '$' };
  ret.extend(std::iter::repeat_n(sigil, dollar_count));
  std::borrow::Cow::Owned(ret.into_iter().collect())
}

pub(super) struct AliasVisitor {
  pub(super) aliases: &'static [&'static str],
}

impl Visitor<'_> for AliasVisitor {
  type Value = &'static str;

  fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
    write!(f, "one of {:?}", self.aliases)
  }

  fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
  where
    E: de::Error,
  {
    self
      .aliases
      .iter()
      .copied()
      .find(|&a| v.eq_ignore_ascii_case(a))
      .ok_or_else(|| de::Error::invalid_value(de::Unexpected::Str(v), &self))
  }
}
