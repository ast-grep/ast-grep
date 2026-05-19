use assert_cmd::Command;
use base64::engine::general_purpose::STANDARD as B64;
use base64::Engine;
use predicates::str::contains;
use tempfile::TempDir;

fn sg() -> Command {
  Command::cargo_bin("ast-grep").expect("ast-grep binary exists")
}

fn extract_state_json(stdout: &str) -> serde_json::Value {
  let url = stdout
    .lines()
    .find(|l| l.starts_with("https://ast-grep.github.io/playground.html#"))
    .expect("stdout contains the playground URL");
  let frag = url.split_once('#').unwrap().1;
  let bytes = B64.decode(frag).expect("valid base64");
  serde_json::from_slice(&bytes).expect("valid json")
}

#[test]
fn playground_with_only_file_prints_url() {
  let dir = TempDir::new().unwrap();
  let file = dir.path().join("hello.ts");
  std::fs::write(&file, "const x: number = 1;\n").unwrap();

  let assert = sg()
    .args(["playground", "--file"])
    .arg(&file)
    .arg("--print")
    .assert()
    .success()
    .stdout(contains("https://ast-grep.github.io/playground.html#"));

  let stdout = String::from_utf8(assert.get_output().stdout.clone()).unwrap();
  let state = extract_state_json(&stdout);
  assert_eq!(state["mode"], "Patch");
  assert_eq!(state["lang"], "typescript");
  assert_eq!(state["source"], "const x: number = 1;\n");
  assert_eq!(state["config"], "");
}

#[test]
fn playground_with_rule_file_loads_yaml() {
  let dir = TempDir::new().unwrap();
  let rule_path = dir.path().join("rule.yml");
  let yaml = "id: no-console\nlanguage: typescript\nrule:\n  pattern: console.log($A)\n";
  std::fs::write(&rule_path, yaml).unwrap();

  let assert = sg()
    .args(["playground", "--rule-file"])
    .arg(&rule_path)
    .arg("--print")
    .assert()
    .success();

  let stdout = String::from_utf8(assert.get_output().stdout.clone()).unwrap();
  let state = extract_state_json(&stdout);
  assert_eq!(state["mode"], "Config");
  assert_eq!(state["lang"], "typescript");
  assert_eq!(state["source"], "");
  assert_eq!(state["config"], yaml);
}

#[test]
fn playground_with_rule_id_in_project() {
  let dir = TempDir::new().unwrap();
  std::fs::create_dir_all(dir.path().join("rules")).unwrap();
  std::fs::write(dir.path().join("sgconfig.yml"), "ruleDirs:\n  - rules\n").unwrap();
  let rule_yaml = "id: no-console\nlanguage: typescript\nrule:\n  pattern: console.log($A)\n";
  std::fs::write(dir.path().join("rules/no-console.yml"), rule_yaml).unwrap();

  let assert = sg()
    .current_dir(dir.path())
    .args(["playground", "--rule", "no-console", "--print"])
    .assert()
    .success();

  let stdout = String::from_utf8(assert.get_output().stdout.clone()).unwrap();
  let state = extract_state_json(&stdout);
  assert_eq!(state["mode"], "Config");
  assert_eq!(state["lang"], "typescript");
  assert!(state["config"]
    .as_str()
    .unwrap()
    .contains("console.log($A)"));
}

#[test]
fn playground_no_inputs_errors() {
  sg()
    .args(["playground", "--print"])
    .assert()
    .failure()
    .stderr(contains("nothing to share"));
}

#[test]
fn playground_rule_and_rule_file_are_exclusive() {
  let dir = TempDir::new().unwrap();
  let path = dir.path().join("r.yml");
  std::fs::write(&path, "id: r\nlanguage: javascript\nrule:\n  pattern: x\n").unwrap();

  sg()
    .args(["playground", "--rule", "r", "--rule-file"])
    .arg(&path)
    .args(["--print"])
    .assert()
    .failure()
    .stderr(contains("cannot be used with"));
}
