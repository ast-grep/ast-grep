use std::fs::read_to_string;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::str::FromStr;

use ansi_term::{Color, Style};
use anyhow::{anyhow, Result};
use ast_grep_config::RuleConfig;
use ast_grep_language::SupportLang;
use base64::engine::general_purpose::STANDARD as B64;
use base64::Engine;
use clap::Args;
use crossterm::tty::IsTty;
use serde::{Deserialize, Serialize};
use serde_yaml::Value;
use std::collections::HashSet;

use crate::config::ProjectConfig;
use crate::lang::SgLang;
use crate::utils::RuleOverwrite;

const PLAYGROUND_BASE: &str = "https://ast-grep.github.io/playground.html";

/// `ansi_term` styles applied to the `--print`-less stderr status lines.
#[derive(Default)]
struct StatusStyles {
  info: Style,
  success: Style,
  warning: Style,
}

impl StatusStyles {
  /// Pick colored styles when stderr is a TTY and `NO_COLOR`/`TERM=dumb`
  /// aren't set; fall back to plain `Style::default()` otherwise.
  fn auto() -> Self {
    let no_color = std::env::var_os("NO_COLOR").is_some();
    let dumb = std::env::var_os("TERM")
      .map(|t| t == "dumb")
      .unwrap_or(false);
    if no_color || dumb || !std::io::stderr().is_tty() {
      Self::default()
    } else {
      Self {
        info: Style::new().dimmed(),
        success: Style::new().fg(Color::Green),
        warning: Style::new().fg(Color::Yellow),
      }
    }
  }
}

/// State shared with the web playground via the URL fragment.
///
/// Mirrors the `State` shape declared in
/// `website/src/components/astGrep/state.ts`; the frontend merges this over
/// its `defaultState`, so fields we leave empty get filled in.
#[derive(Serialize, Default, Debug)]
pub struct PlaygroundState {
  pub mode: String,
  pub query: String,
  pub rewrite: String,
  pub config: String,
  pub source: String,
  pub strictness: String,
  pub selector: String,
  pub lang: String,
}

/// Encode the state as `base64(utf8(json(state)))` and append it to the
/// playground URL as a fragment. The encoding is bit-compatible with the
/// frontend's `btoa(unescape(encodeURIComponent(JSON.stringify(state))))`.
pub fn build_url(state: &PlaygroundState) -> String {
  let json = serde_json::to_string(state).expect("PlaygroundState is always serializable");
  let fragment = B64.encode(json.as_bytes());
  format!("{PLAYGROUND_BASE}#{fragment}")
}

/// Pick the playground `lang` field, preferring `--lang` > the rule's
/// `language` > the file's extension. Errors when the resolved language is
/// not supported by the web playground (Dart / Haskell / Solidity / custom).
pub(crate) fn resolve_lang(
  flag: Option<&str>,
  rule_lang: Option<SgLang>,
  file: Option<&Path>,
) -> Result<String> {
  if let Some(s) = flag {
    let lang = SgLang::from_str(s).map_err(|_| anyhow!("unsupported --lang value: {s}"))?;
    return playground_lang_name(lang);
  }
  if let Some(lang) = rule_lang {
    return playground_lang_name(lang);
  }
  if let Some(path) = file {
    if let Some(lang) = SgLang::from_path(path) {
      return playground_lang_name(lang);
    }
  }
  Err(anyhow!("could not infer language; pass --lang explicitly"))
}

/// Map an `SgLang` to the lowercase identifier the playground expects, or
/// error if the language isn't in `SupportedLang` on the web side.
fn playground_lang_name(lang: SgLang) -> Result<String> {
  let name: &'static str = match lang {
    SgLang::Builtin(builtin) => match builtin {
      SupportLang::Bash => "bash",
      SupportLang::C => "c",
      SupportLang::Cpp => "cpp",
      SupportLang::CSharp => "csharp",
      SupportLang::Css => "css",
      SupportLang::Elixir => "elixir",
      SupportLang::Go => "go",
      SupportLang::Hcl => "hcl",
      SupportLang::Html => "html",
      SupportLang::Java => "java",
      SupportLang::JavaScript => "javascript",
      SupportLang::Json => "json",
      SupportLang::Kotlin => "kotlin",
      SupportLang::Lua => "lua",
      SupportLang::Nix => "nix",
      SupportLang::Php => "php",
      SupportLang::Python => "python",
      SupportLang::Ruby => "ruby",
      SupportLang::Rust => "rust",
      SupportLang::Scala => "scala",
      SupportLang::Swift => "swift",
      SupportLang::Tsx => "tsx",
      SupportLang::TypeScript => "typescript",
      SupportLang::Yaml => "yaml",
      SupportLang::Dart | SupportLang::Haskell | SupportLang::Solidity => {
        return Err(unsupported_playground_lang(lang));
      }
    },
    SgLang::Custom(_) => return Err(unsupported_playground_lang(lang)),
  };
  Ok(name.into())
}

/// Build the consistent "language X not supported" error used wherever the
/// playground's `SupportedLang` set is consulted.
fn unsupported_playground_lang(lang: SgLang) -> anyhow::Error {
  let name = lang.to_string().to_lowercase();
  anyhow!("language '{name}' is not supported by the web playground")
}

/// Read the source file into a string, or return an empty string when no
/// file was supplied (i.e. user is opening the playground with rule only).
pub(crate) fn resolve_source(file: Option<&Path>) -> Result<String> {
  match file {
    None => Ok(String::new()),
    Some(path) => read_to_string(path)
      .map_err(|e| anyhow!("failed to read source file {}: {e}", path.display())),
  }
}

/// Result of rule resolution: (YAML text, optional language).
type ResolvedRule = (String, Option<SgLang>);

/// Resolve `--rule-file` or `--rule <ID>` into the YAML text the playground
/// should display along with the inferred language. Returns `None` when the
/// user passed only `--file`.
pub(crate) fn resolve_rule(
  rule_id: Option<&str>,
  rule_file: Option<&Path>,
  project: Option<&ProjectConfig>,
) -> Result<Option<ResolvedRule>> {
  if let Some(path) = rule_file {
    let yaml = read_to_string(path)
      .map_err(|e| anyhow!("failed to read rule file {}: {e}", path.display()))?;
    let lang = infer_rule_file_lang(&yaml)?;
    return Ok(Some((yaml, lang)));
  }
  if let Some(id) = rule_id {
    let project = project.ok_or_else(|| {
      anyhow!("--rule '{id}' requires a project config (sgconfig.yml) - not found")
    })?;
    let (collection, _trace) = project
      .find_rules(RuleOverwrite::default())
      .map_err(|e| anyhow!("failed to load project rules: {e}"))?;
    let rule = collection
      .get_rule(id)
      .ok_or_else(|| anyhow!("rule id '{id}' not found in project config"))?;
    let value = rule_to_yaml_value(rule)?;
    ensure_rule_self_contained(&value, &rule.id)?;
    let yaml = serde_yaml::to_string(&value)
      .map_err(|e| anyhow!("failed to serialize rule '{}' to YAML: {e}", rule.id))?;
    return Ok(Some((yaml, Some(rule.language))));
  }
  Ok(None)
}

/// Extract the `language:` field from each YAML document in `--rule-file`
/// content without going through the full rule deserializer — which would
/// otherwise fail when a rule references project utilities not loaded here.
/// For multi-document YAML the last document's language wins.
fn infer_rule_file_lang(yaml: &str) -> Result<Option<SgLang>> {
  let mut lang = None;
  for doc in serde_yaml::Deserializer::from_str(yaml) {
    let value =
      Value::deserialize(doc).map_err(|e| anyhow!("failed to parse rule file YAML: {e}"))?;
    let Some(map) = value.as_mapping() else {
      continue;
    };
    let key = Value::String("language".into());
    let Some(value) = map.get(&key) else {
      continue;
    };
    lang = Some(
      SgLang::deserialize(value.clone())
        .map_err(|e| anyhow!("failed to parse rule file language: {e}"))?,
    );
  }
  Ok(lang)
}

/// Serialize a `RuleConfig` to a `serde_yaml::Value`, then drop empty
/// optional fields so the playground sees a clean rule body.
fn rule_to_yaml_value(rule: &RuleConfig<SgLang>) -> Result<Value> {
  let mut value = serde_yaml::to_value(&**rule)
    .map_err(|e| anyhow!("failed to serialize rule '{}' to YAML: {e}", rule.id))?;
  strip_serialized_null_fields(&mut value);
  Ok(value)
}

/// Recursively drop `key: null` entries from a YAML value, but only for keys
/// in [`is_serialized_optional_key`]. The contents of `metadata:` are
/// passed through untouched since it's user-controlled free-form data.
fn strip_serialized_null_fields(value: &mut Value) {
  match value {
    Value::Mapping(map) => {
      map.retain(|key, value| !(value.is_null() && is_serialized_optional_key(key)));
      for (key, value) in map.iter_mut() {
        if key.as_str() != Some("metadata") {
          strip_serialized_null_fields(value);
        }
      }
    }
    Value::Sequence(seq) => {
      for value in seq {
        strip_serialized_null_fields(value);
      }
    }
    Value::Tagged(tagged) => strip_serialized_null_fields(&mut tagged.value),
    Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => {}
  }
}

/// `Option<...>` fields on `SerializableRuleConfig` / `SerializableRuleCore`
/// that should be elided when serialized to `null`. Driven from the schema
/// in `crates/config/src/rule_config.rs`; keep in sync if a new optional
/// field is added there.
fn is_serialized_optional_key(key: &Value) -> bool {
  matches!(
    key.as_str(),
    Some(
      "constraints"
        | "field"
        | "files"
        | "fix"
        | "ignores"
        | "labels"
        | "metadata"
        | "note"
        | "ofRule"
        | "rewriters"
        | "selector"
        | "strictness"
        | "transform"
        | "url"
        | "utils"
    )
  )
}

/// Project-level utility rules are not serialized into playground URLs.
/// Project rules must therefore be self-contained: every `matches:` reference
/// needs a same-file local `utils:` entry.
fn ensure_rule_self_contained(rule: &Value, rule_id: &str) -> Result<()> {
  let local_utils = local_util_ids(rule)?;
  let mut refs = Vec::new();
  collect_match_refs(rule, &mut refs);

  let mut seen = HashSet::new();
  let external: Vec<_> = refs
    .into_iter()
    .filter(|id| !local_utils.contains(id))
    .filter(|id| seen.insert(id.clone()))
    .collect();

  if external.is_empty() {
    return Ok(());
  }

  Err(anyhow!(
    "rule '{}' references project utilities ({}) which cannot be shared with the playground; use a self-contained rule file or inline them as local utils",
    rule_id,
    external.join(", ")
  ))
}

fn local_util_ids(rule: &Value) -> Result<HashSet<String>> {
  let rule = rule
    .as_mapping()
    .ok_or_else(|| anyhow!("serialized rule must be a YAML mapping"))?;
  let Some(utils) = rule.get(&Value::String("utils".into())) else {
    return Ok(HashSet::new());
  };
  match utils {
    Value::Mapping(utils) => Ok(utils
      .keys()
      .filter_map(Value::as_str)
      .map(str::to_string)
      .collect()),
    Value::Null => Ok(HashSet::new()),
    _ => Err(anyhow!("serialized rule has invalid `utils` field")),
  }
}

/// Walk the rule YAML and collect every `matches:` reference: either a bare
/// id string or the keys of a `{ utilId: { ... args ... } }` call form.
fn collect_match_refs(value: &Value, refs: &mut Vec<String>) {
  match value {
    Value::Mapping(map) => {
      for (key, value) in map {
        if key.as_str() == Some("matches") {
          collect_match_ids(value, refs);
          continue;
        }
        if key.as_str() == Some("metadata") {
          continue;
        }
        collect_match_refs(value, refs);
      }
    }
    Value::Sequence(seq) => {
      for value in seq {
        collect_match_refs(value, refs);
      }
    }
    Value::Tagged(tagged) => collect_match_refs(&tagged.value, refs),
    Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => {}
  }
}

/// Push the ids referenced by a `matches:` value. Sequence values are not a
/// valid `matches:` shape in ast-grep's schema, so they are skipped.
fn collect_match_ids(value: &Value, refs: &mut Vec<String>) {
  match value {
    Value::String(id) => {
      refs.push(id.clone());
    }
    Value::Mapping(calls) => {
      for (callee, _args) in calls {
        if let Some(id) = callee.as_str() {
          refs.push(id.to_string());
        }
      }
    }
    Value::Tagged(tagged) => collect_match_ids(&tagged.value, refs),
    Value::Null | Value::Bool(_) | Value::Number(_) | Value::Sequence(_) => {}
  }
}

/// Arguments accepted by the `ast-grep playground` subcommand.
#[derive(Args)]
pub struct PlaygroundArg {
  /// Source file to load into the playground.
  #[clap(short, long, value_name = "FILE")]
  pub file: Option<PathBuf>,

  /// Rule ID to look up in the project config and load into the playground.
  #[clap(short, long, value_name = "ID", conflicts_with = "rule_file")]
  pub rule: Option<String>,

  /// Read rule YAML from a path (alternative to --rule).
  #[clap(long, value_name = "PATH")]
  pub rule_file: Option<PathBuf>,

  /// Language override. Inferred from --file extension or rule's language otherwise.
  #[clap(short, long, value_name = "LANG")]
  pub lang: Option<String>,

  /// Print the URL only; do not open the browser.
  #[clap(long)]
  pub print: bool,
}

/// Entry point for `ast-grep playground`: resolve the requested rule and/or
/// source, build the playground URL, then either open it in the user's
/// default browser or print it (when `--print` is set, or as a fallback
/// when opening the browser fails).
pub fn run_playground(arg: PlaygroundArg, project: Result<ProjectConfig>) -> Result<ExitCode> {
  if arg.file.is_none() && arg.rule.is_none() && arg.rule_file.is_none() {
    return Err(anyhow!(
      "nothing to share - pass --file and/or --rule (or --rule-file)"
    ));
  }

  let project_ref = project.as_ref().ok();

  let (config_yaml, rule_lang) =
    match resolve_rule(arg.rule.as_deref(), arg.rule_file.as_deref(), project_ref)? {
      Some((yaml, lang)) => (yaml, lang),
      None => (String::new(), None),
    };

  let source = resolve_source(arg.file.as_deref())?;
  let lang = resolve_lang(arg.lang.as_deref(), rule_lang, arg.file.as_deref())?;

  let mode = if config_yaml.is_empty() {
    "Patch"
  } else {
    "Config"
  };
  let state = PlaygroundState {
    mode: mode.into(),
    lang,
    source,
    config: config_yaml,
    ..PlaygroundState::default()
  };
  let url = build_url(&state);

  if arg.print {
    println!("{url}");
    return Ok(ExitCode::SUCCESS);
  }

  let styles = StatusStyles::auto();
  eprintln!(
    "{}",
    styles.info.paint("Opening playground in your browser...")
  );
  match open::that(&url) {
    Ok(()) => {
      eprintln!("{}", styles.success.paint("Playground opened in browser."));
    }
    Err(e) => {
      eprintln!(
        "{} {e}",
        styles
          .warning
          .paint("Could not open the browser automatically:"),
      );
      eprintln!("Open this URL manually:");
      println!("{url}");
    }
  }

  Ok(ExitCode::SUCCESS)
}

#[cfg(test)]
mod tests {
  use super::*;
  use ast_grep_config::{from_yaml_string, GlobalRules};
  use std::path::Path;
  use std::str::FromStr;

  fn serialize_rule_yaml(rule: &RuleConfig<SgLang>) -> String {
    let value = rule_to_yaml_value(rule).expect("rule serializes");
    serde_yaml::to_string(&value).expect("yaml serializes")
  }

  fn decode_state(url: &str) -> serde_json::Value {
    let frag = url.split_once('#').expect("url has fragment").1;
    let bytes = B64.decode(frag).expect("valid base64");
    serde_json::from_slice(&bytes).expect("valid json")
  }

  #[test]
  fn build_url_encodes_state_round_trip() {
    let state = PlaygroundState {
      mode: "Config".into(),
      lang: "typescript".into(),
      source: "console.log(1)".into(),
      config: "rule:\n  pattern: console.log($A)\n".into(),
      ..PlaygroundState::default()
    };
    let url = build_url(&state);
    assert!(url.starts_with("https://ast-grep.github.io/playground.html#"));
    let decoded = decode_state(&url);
    assert_eq!(decoded["mode"], "Config");
    assert_eq!(decoded["lang"], "typescript");
    assert_eq!(decoded["source"], "console.log(1)");
    assert_eq!(decoded["config"], "rule:\n  pattern: console.log($A)\n");
    assert_eq!(decoded["query"], "");
    assert_eq!(decoded["rewrite"], "");
    assert_eq!(decoded["strictness"], "");
    assert_eq!(decoded["selector"], "");
  }

  #[test]
  fn build_url_handles_unicode_source() {
    let state = PlaygroundState {
      mode: "Patch".into(),
      lang: "javascript".into(),
      source: "// 你好 🦀".into(),
      ..PlaygroundState::default()
    };
    let url = build_url(&state);
    let decoded = decode_state(&url);
    assert_eq!(decoded["source"], "// 你好 🦀");
  }

  #[test]
  fn resolve_lang_prefers_explicit_flag() {
    let got = resolve_lang(
      Some("typescript"),
      Some(SgLang::from_str("rust").unwrap()),
      Some(Path::new("foo.js")),
    )
    .unwrap();
    assert_eq!(got, "typescript");
  }

  #[test]
  fn resolve_lang_falls_back_to_rule_language() {
    let got = resolve_lang(
      None,
      Some(SgLang::from_str("rust").unwrap()),
      Some(Path::new("foo.js")),
    )
    .unwrap();
    assert_eq!(got, "rust");
  }

  #[test]
  fn resolve_lang_falls_back_to_file_extension() {
    let got = resolve_lang(None, None, Some(Path::new("foo.tsx"))).unwrap();
    assert_eq!(got, "tsx");
  }

  #[test]
  fn resolve_lang_errors_when_nothing_known() {
    let err = resolve_lang(None, None, None).unwrap_err();
    assert!(err.to_string().to_lowercase().contains("--lang"));
  }

  #[test]
  fn resolve_lang_normalises_explicit_alias_to_lowercase() {
    let got = resolve_lang(Some("JS"), None, None).unwrap();
    assert_eq!(got, "javascript");
  }

  #[test]
  fn resolve_lang_rejects_playground_unsupported_lang() {
    let cases: &[(&str, Option<&str>, Option<&Path>)] = &[
      ("explicit flag", Some("dart"), None),
      ("file extension", None, Some(Path::new("main.dart"))),
    ];
    for (desc, flag, file) in cases {
      let err = resolve_lang(*flag, None, *file).unwrap_err();
      let msg = err.to_string();
      assert!(msg.contains("dart"), "{desc}: {msg}");
      assert!(msg.contains("playground"), "{desc}: {msg}");
    }
  }

  #[test]
  fn resolve_source_reads_file_contents() {
    let tmp = tempfile::NamedTempFile::new().unwrap();
    std::fs::write(tmp.path(), "let x = 1;").unwrap();
    let got = resolve_source(Some(tmp.path())).unwrap();
    assert_eq!(got, "let x = 1;");
  }

  #[test]
  fn resolve_source_returns_empty_when_no_file() {
    let got = resolve_source(None).unwrap();
    assert_eq!(got, "");
  }

  #[test]
  fn resolve_source_errors_on_missing_file() {
    let err = resolve_source(Some(Path::new("/definitely/nonexistent/path"))).unwrap_err();
    assert!(err.to_string().to_lowercase().contains("read"));
  }

  #[test]
  fn resolve_rule_returns_none_when_no_input() {
    let got = resolve_rule(None, None, None).unwrap();
    assert!(got.is_none());
  }

  #[test]
  fn resolve_rule_reads_rule_file_verbatim() {
    let tmp = tempfile::NamedTempFile::new().unwrap();
    let yaml = "id: test\nlanguage: javascript\nrule:\n  pattern: foo($A)\n";
    std::fs::write(tmp.path(), yaml).unwrap();
    let (text, lang) = resolve_rule(None, Some(tmp.path()), None).unwrap().unwrap();
    assert_eq!(text, yaml);
    assert_eq!(lang.unwrap(), SgLang::from_str("javascript").unwrap());
  }

  #[test]
  fn resolve_rule_reads_rule_file_language_without_validating_global_utils() {
    let tmp = tempfile::NamedTempFile::new().unwrap();
    let yaml = "id: test\nlanguage: javascript\nrule:\n  matches: project-util\n";
    std::fs::write(tmp.path(), yaml).unwrap();

    let (text, lang) = resolve_rule(None, Some(tmp.path()), None).unwrap().unwrap();

    assert_eq!(text, yaml);
    assert_eq!(lang.unwrap(), SgLang::from_str("javascript").unwrap());
  }

  #[test]
  fn resolve_rule_keeps_local_utils_self_contained() {
    let temp = tempfile::tempdir().unwrap();
    std::fs::create_dir(temp.path().join("rules")).unwrap();
    std::fs::write(
      temp.path().join("rules/use-num.yml"),
      "id: use-num\nlanguage: javascript\nrule:\n  matches: num\nutils:\n  num:\n    kind: number\n",
    )
    .unwrap();
    let project = ProjectConfig {
      project_dir: temp.path().to_path_buf(),
      rule_dirs: vec![PathBuf::from("rules")],
      test_configs: None,
      util_dirs: None,
    };

    let (text, lang) = resolve_rule(Some("use-num"), None, Some(&project))
      .unwrap()
      .unwrap();

    assert_eq!(lang.unwrap(), SgLang::from_str("javascript").unwrap());
    assert!(text.contains("utils:"));
    assert!(text.contains("num:"));
    assert!(text.contains("kind: number"));
    from_yaml_string::<SgLang>(&text, &GlobalRules::default()).expect("shared YAML parses alone");
  }

  #[test]
  fn resolve_rule_errors_on_project_util_reference() {
    let temp = tempfile::tempdir().unwrap();
    std::fs::create_dir(temp.path().join("rules")).unwrap();
    std::fs::create_dir(temp.path().join("utils")).unwrap();
    std::fs::write(
      temp.path().join("utils/num.yml"),
      "id: num\nlanguage: javascript\nrule:\n  kind: number\n",
    )
    .unwrap();
    std::fs::write(
      temp.path().join("rules/use-num.yml"),
      "id: use-num\nlanguage: javascript\nrule:\n  matches: num\n",
    )
    .unwrap();
    let project = ProjectConfig {
      project_dir: temp.path().to_path_buf(),
      rule_dirs: vec![PathBuf::from("rules")],
      test_configs: None,
      util_dirs: Some(vec![PathBuf::from("utils")]),
    };

    let err = resolve_rule(Some("use-num"), None, Some(&project)).unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("project utilities"), "{msg}");
    assert!(msg.contains("num"), "{msg}");
  }

  #[test]
  fn resolve_rule_errors_when_id_missing_and_no_project() {
    let err = resolve_rule(Some("does-not-exist"), None, None).unwrap_err();
    assert!(err.to_string().contains("does-not-exist"));
  }

  fn parse_single_rule(yaml: &str) -> RuleConfig<SgLang> {
    from_yaml_string::<SgLang>(yaml, &GlobalRules::default())
      .expect("parses")
      .pop()
      .expect("has one rule")
  }

  #[test]
  fn serialize_rule_yaml_strips_null_lines() {
    let rule = parse_single_rule("id: t\nlanguage: javascript\nrule:\n  pattern: foo\n");
    let out = serialize_rule_yaml(&rule);
    assert!(
      !out.contains(": null"),
      "stripped output should not contain ': null': {out}"
    );
    assert!(out.contains("id: t"));
    assert!(out.contains("pattern: foo"));
  }

  #[test]
  fn serialize_rule_yaml_preserves_null_like_lines_in_scalars() {
    let rule = parse_single_rule(
      "id: t\nlanguage: javascript\nnote: |\n  foo: null\nrule:\n  pattern: foo\n",
    );
    let out = serialize_rule_yaml(&rule);
    assert!(out.contains("foo: null"), "scalar line was dropped: {out}");
  }

  #[test]
  fn run_playground_errors_when_no_input() {
    let arg = PlaygroundArg {
      file: None,
      rule: None,
      rule_file: None,
      lang: None,
      print: true,
    };
    let err = run_playground(arg, Err(anyhow!("no project"))).unwrap_err();
    assert!(err.to_string().contains("nothing to share"));
  }
}
