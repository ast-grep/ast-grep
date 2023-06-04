use crate::language::Language;
use crate::meta_var::{split_first_meta_var, MetaVarEnv};
use crate::Doc;

// replace meta_var in template string, e.g. "Hello $NAME" -> "Hello World"
// TODO: use Cow instead of String
pub fn replace_meta_var_in_string<D: Doc>(
  mut template: &str,
  env: &MetaVarEnv<D>,
  lang: &D::Lang,
) -> String {
  let mv_char = lang.meta_var_char();
  let mut ret = String::new();
  while let Some(i) = template.find(mv_char) {
    ret.push_str(&template[..i]);
    template = &template[i..];
    let (meta_var, remaining) = split_first_meta_var(template, mv_char);
    if let Some(n) = env.get_match(meta_var) {
      ret.push_str(&n.text());
    }
    template = remaining;
  }
  ret.push_str(template);
  ret
}
