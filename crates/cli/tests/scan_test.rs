mod common;

use std::process::ExitCode;

use anyhow::Result;
use assert_cmd::{cargo_bin, Command};
use ast_grep::main_with_args;
use common::create_test_files;
use predicates::prelude::*;
use predicates::str::contains;
use serde_json::{from_slice, Value};
use tempfile::TempDir;

const CONFIG: &str = "
ruleDirs:
- rules
testConfigs:
- testDir: rule-tests
";
const RULE1: &str = "
id: on-rule
message: test rule
severity: warning
language: TypeScript
rule:
  pattern: Some($A)
";

const RULE2: &str = "
id: off-rule
severity: off
language: TypeScript
rule:
  pattern: Some($A)
";

fn setup() -> Result<TempDir> {
  let dir = create_test_files([
    ("sgconfig.yml", CONFIG),
    ("rules/on-rule.yml", RULE1),
    ("rules/off-rule.yml", RULE2),
    ("test.ts", "Some(123)"),
  ])?;
  assert!(dir.path().join("sgconfig.yml").exists());
  Ok(dir)
}

fn sg(s: &str) -> Result<ExitCode> {
  let args = s.split(' ').map(String::from);
  main_with_args(args)
}

#[test]
fn test_sg_scan() -> Result<()> {
  let dir = setup()?;
  let config = dir.path().join("sgconfig.yml");
  let ret = sg(&format!("ast-grep scan -c {}", config.display()));
  assert!(ret.is_ok());
  let ret = sg(&format!("ast-grep scan -c={}", config.display()));
  assert!(ret.is_ok());
  let ret = sg(&format!("ast-grep scan --config {}", config.display()));
  assert!(ret.is_ok());
  let ret = sg(&format!("ast-grep scan --config={}", config.display()));
  assert!(ret.is_ok());
  drop(dir);
  Ok(())
}

#[test]
fn test_sg_rule_off() -> Result<()> {
  let dir = setup()?;
  Command::new(cargo_bin!())
    .current_dir(dir.path())
    .args(["scan"])
    .assert()
    .success()
    .stdout(contains("on-rule"))
    .stdout(contains("off-rule").not());
  drop(dir);
  Ok(())
}

#[test]
fn test_sg_scan_inline_rules() -> Result<()> {
  let inline_rules = "{id: test, language: ts, rule: {pattern: console.log($A)}}";
  Command::new(cargo_bin!())
    .args(["scan", "--stdin", "--inline-rules", inline_rules, "--json"])
    .write_stdin("console.log(123)")
    .assert()
    .stdout(contains("\"text\": \"console.log(123)\""))
    .stdout(predicate::function(|n| from_slice::<Value>(n).is_ok()));
  Ok(())
}

const MULTI_RULES: &str = "
id: rule-1
language: TypeScript
rule: { pattern: Some($A) }
---
id: rule-2
language: TypeScript
rule: { pattern: None }
";

#[test]
fn test_sg_scan_multiple_rules_in_one_file() -> Result<()> {
  let dir = create_test_files([("rule.yml", MULTI_RULES), ("test.ts", "Some(123) + None")])?;
  Command::new(cargo_bin!())
    .current_dir(dir.path())
    .args(["scan", "-r", "rule.yml"])
    .assert()
    .success()
    .stdout(contains("rule-1"))
    .stdout(contains("rule-2"))
    .stdout(contains("rule-3").not());
  Ok(())
}

// see #517, #668
#[test]
fn test_sg_scan_py_empty_text() -> Result<()> {
  let inline_rules = "{id: test, language: py, rule: {pattern: None}}";
  Command::new(cargo_bin!())
    .args(["scan", "--stdin", "--inline-rules", inline_rules])
    .write_stdin("\n\n\n\n\nNone")
    .assert()
    .stdout(contains("STDIN:6:1"));
  Ok(())
}

#[test]
fn test_sg_scan_html() -> Result<()> {
  let dir = create_test_files([
    ("rule.yml", RULE1),
    ("test.html", "<script lang=ts>Some(123)</script>"),
  ])?;
  Command::new(cargo_bin!())
    .current_dir(dir.path())
    .args(["scan", "-r", "rule.yml", "--inspect=summary"])
    .assert()
    .success()
    .stdout(contains("on-rule"))
    .stdout(contains("script"))
    .stdout(contains("rule-3").not())
    .stderr(contains("scannedFileCount=1"));
  Ok(())
}

#[test]
fn test_scan_unused_suppression() -> Result<()> {
  let dir = create_test_files([
    ("sgconfig.yml", CONFIG),
    ("rules/rule.yml", RULE1),
    ("test.ts", "None(123) // ast-grep-ignore"),
  ])?;
  Command::new(cargo_bin!())
    .current_dir(dir.path())
    .args(["scan"])
    .assert()
    .success()
    .stdout(contains("unused-suppression"));
  Ok(())
}

#[test]
fn test_unused_suppression_only_in_scan() -> Result<()> {
  let dir = create_test_files([
    ("sgconfig.yml", CONFIG),
    ("rules/rule.yml", RULE1),
    ("test.ts", "None(123) // ast-grep-ignore"),
  ])?;
  Command::new(cargo_bin!())
    .current_dir(dir.path())
    .args(["scan", "-r", "rules/rule.yml"])
    .assert()
    .success()
    .stdout(contains("unused-suppression").not());
  Command::new(cargo_bin!())
    .current_dir(dir.path())
    .args(["scan", "--filter", "on-rule"])
    .assert()
    .success()
    .stdout(contains("unused-suppression").not());
  Command::new(cargo_bin!())
    .current_dir(dir.path())
    .args(["scan", "--off", "on-rule"])
    .assert()
    .success()
    .stdout(contains("unused-suppression").not());
  Command::new(cargo_bin!())
    .current_dir(dir.path())
    .args(["scan", "--inline-rules", RULE1])
    .assert()
    .success()
    .stdout(contains("unused-suppression").not());
  Ok(())
}

#[test]
fn test_scan_unused_suppression_off() -> Result<()> {
  let dir = create_test_files([
    ("sgconfig.yml", CONFIG),
    ("rules/rule.yml", RULE1),
    ("test.ts", "None(123) // ast-grep-ignore"),
  ])?;
  Command::new(cargo_bin!())
    .current_dir(dir.path())
    .args(["scan", "--off"])
    .assert()
    .success();
  Ok(())
}

#[test]
fn test_severity_override() -> Result<()> {
  let dir = setup()?;
  Command::new(cargo_bin!())
    .current_dir(dir.path())
    .args(["scan", "--error"])
    .assert()
    .failure()
    .stdout(contains("error"));
  Command::new(cargo_bin!())
    .current_dir(dir.path())
    .args(["scan", "--error=on-rule"])
    .assert()
    .failure()
    .stdout(contains("error"));
  Command::new(cargo_bin!())
    .current_dir(dir.path())
    .args(["scan", "--error=not-exist"])
    .assert()
    .success()
    .stdout(contains("warning"));
  Ok(())
}

const PY_RULE: &str = r"
id: transform-indent
language: python
rule: { pattern: 'class $CN(): $A' }
transform:
  AR:
    substring: { source: $A }
fix: |-
  class $CN():
      $AR
";

const PY_FILE: &str = r"
if something:
    class B():
        def replace(self):
        print(self1)
";

#[test]
fn test_transform_indent() -> Result<()> {
  let dir = create_test_files([
    ("sgconfig.yml", CONFIG),
    ("rules/rule.yml", PY_RULE),
    ("test.py", PY_FILE),
  ])?;
  Command::new(cargo_bin!())
    .current_dir(dir.path())
    .args(["scan"])
    .assert()
    .success()
    .stdout(contains("print").not())
    .stdout(contains("transform-indent"));
  Ok(())
}

const LABEL_RULE: &str = r"
id: label-test
language: TypeScript
rule: { all: [pattern: Some($A), pattern: $B] }
labels:
  A:
    style: primary
    message: primary-label
  B:
    style: secondary
    message: secondary-label
";

#[test]
fn test_label() -> Result<()> {
  let dir = create_test_files([
    ("sgconfig.yml", CONFIG),
    ("rules/rule.yml", LABEL_RULE),
    ("test.ts", "Some(123) + None"),
  ])?;
  Command::new(cargo_bin!())
    .current_dir(dir.path())
    .args(["scan"])
    .assert()
    .success()
    .stdout(contains("primary-label"))
    .stdout(contains("secondary-label"))
    .stdout(contains(" -----^^^-")) // a label range test
    .stdout(contains(" -----^^^--").not());
  Ok(())
}
const FILE_RULE: &str = "
id: file-rule
message: test rule
language: TypeScript
rule: { pattern: Some($A) }
files: [ test/*.ts ]
";

#[test]
fn test_file() -> Result<()> {
  let dir = create_test_files([
    ("sgconfig.yml", CONFIG),
    ("rules/rule.yml", FILE_RULE),
    ("test/hit.ts", "Some(123)"),
    ("not.ts", "Some(456)"),
  ])?;
  Command::new(cargo_bin!())
    .current_dir(dir.path().join("test"))
    .args(["scan"])
    .assert()
    .success()
    .stdout(contains("hit.ts"))
    .stdout(contains("not.ts").not());
  Command::new(cargo_bin!())
    .current_dir(dir.path().join("test"))
    .args(["scan", "-c", "../sgconfig.yml"])
    .assert()
    .success()
    .stdout(contains("hit.ts"))
    .stdout(contains("not.ts").not());
  Command::new(cargo_bin!())
    .current_dir(dir.path())
    .args(["scan", "-c", "sgconfig.yml"])
    .assert()
    .success()
    .stdout(contains("hit.ts"))
    .stdout(contains("not.ts").not());
  Ok(())
}

const SV_RULE: &str = "
id: sv-display
message: systemverilog rule
language: systemverilog
rule:
  pattern: $display($A);
";

const SV_INST_RULE: &str = "
id: sv-inst
message: systemverilog instantiation rule
language: systemverilog
rule:
  kind: module_instantiation
";

#[test]
fn test_sg_scan_systemverilog() -> Result<()> {
  let dir = create_test_files([
    ("rule.yml", SV_RULE),
    (
      "test.sv",
      "module m; initial begin $display(data); end endmodule",
    ),
    ("test.ts", "console.log(123)"),
  ])?;
  Command::new(cargo_bin!())
    .current_dir(dir.path())
    .args(["scan", "-r", "rule.yml"])
    .assert()
    .success()
    .stdout(contains("sv-display"))
    .stdout(contains("$display(data)"))
    .stdout(contains("console.log(123)").not());
  Ok(())
}

#[test]
fn test_sg_scan_systemverilog_json() -> Result<()> {
  let dir = create_test_files([
    ("rule.yml", SV_RULE),
    (
      "test.sv",
      "module m; initial begin $display(data); end endmodule",
    ),
  ])?;
  Command::new(cargo_bin!())
    .current_dir(dir.path())
    .args(["scan", "-r", "rule.yml", "--json"])
    .assert()
    .success()
    .stdout(contains("\"ruleId\": \"sv-display\""))
    .stdout(contains("\"text\": \"$display(data);\""))
    .stdout(predicate::function(|n| from_slice::<Value>(n).is_ok()));
  Ok(())
}

#[test]
fn test_sg_scan_systemverilog_v_vh_infer_lang() -> Result<()> {
  let dir = create_test_files([
    ("rule.yml", SV_RULE),
    (
      "test.v",
      "module m; initial begin $display(v_data); end endmodule",
    ),
    (
      "test.vh",
      "module h; initial begin $display(vh_data); end endmodule",
    ),
    ("test.ts", "console.log(123)"),
  ])?;
  Command::new(cargo_bin!())
    .current_dir(dir.path())
    .args(["scan", "-r", "rule.yml"])
    .assert()
    .success()
    .stdout(contains("$display(v_data);"))
    .stdout(contains("$display(vh_data);"))
    .stdout(contains("console.log(123)").not());
  Ok(())
}

#[test]
fn test_sg_scan_systemverilog_module_and_interface_instantiation() -> Result<()> {
  let dir = create_test_files([
    ("rule.yml", SV_INST_RULE),
    (
      "test.sv",
      r#"
interface axi_if #(int W = 8) (input logic clk, rst_n);
endinterface

module sub_mod #(parameter int W = 8) (
  input logic clk,
  input logic rst_n,
  input logic [W-1:0] in,
  output logic [W-1:0] out
);
endmodule

module top(input logic clk, rst_n, input logic [7:0] a, output logic [7:0] b);
  axi_if #(8) m_if (.clk(clk), .rst_n(rst_n));
  sub_mod u0 (.clk(clk), .rst_n(rst_n), .in(a), .out(b));
  sub_mod #(.W(8)) u1 (.clk(clk), .rst_n(rst_n), .in(a), .out(b));
  sub_mod u_arr [1:0] (.clk(clk), .rst_n(rst_n), .in(a), .out(b));
  and g1 (b[0], a[0], a[1]);
endmodule
"#,
    ),
  ])?;
  Command::new(cargo_bin!())
    .current_dir(dir.path())
    .args(["scan", "-r", "rule.yml"])
    .assert()
    .success()
    .stdout(contains("axi_if #(8) m_if"))
    .stdout(contains("sub_mod u0"))
    .stdout(contains("sub_mod #(.W(8)) u1"))
    .stdout(contains("sub_mod u_arr [1:0]"))
    .stdout(contains("and g1").not());
  Ok(())
}

#[test]
fn test_sg_scan_systemverilog_instantiation_json_count() -> Result<()> {
  let dir = create_test_files([
    ("rule.yml", SV_INST_RULE),
    (
      "test.sv",
      r#"
interface axi_if #(int W = 8) (input logic clk, rst_n);
endinterface

module sub_mod #(parameter int W = 8) (
  input logic clk,
  input logic rst_n,
  input logic [W-1:0] in,
  output logic [W-1:0] out
);
endmodule

module top(input logic clk, rst_n, input logic [7:0] a, output logic [7:0] b);
  axi_if #(8) m_if (.clk(clk), .rst_n(rst_n));
  sub_mod u0 (.clk(clk), .rst_n(rst_n), .in(a), .out(b));
  sub_mod #(.W(8)) u1 (.clk(clk), .rst_n(rst_n), .in(a), .out(b));
  sub_mod u_arr [1:0] (.clk(clk), .rst_n(rst_n), .in(a), .out(b));
endmodule
"#,
    ),
  ])?;
  let output = Command::new(cargo_bin!())
    .current_dir(dir.path())
    .args(["scan", "-r", "rule.yml", "--json"])
    .assert()
    .success()
    .get_output()
    .stdout
    .clone();
  let json: Value = from_slice(&output)?;
  let matches = json.as_array().expect("should be array");
  assert_eq!(matches.len(), 4, "should match 4 instantiations");
  Ok(())
}

#[test]
fn test_sg_scan_systemverilog_instantiation_variants() -> Result<()> {
  let dir = create_test_files([
    ("rule.yml", SV_INST_RULE),
    (
      "test.sv",
      r#"
interface axi_if #(int W = 8) (input logic clk, rst_n);
endinterface

module sub_mod #(parameter int W = 8) (
  input logic clk,
  input logic rst_n,
  input logic [W-1:0] in,
  output logic [W-1:0] out
);
endmodule

module top(input logic clk, rst_n, input logic [7:0] a, output logic [7:0] b);
  axi_if #(8) m_if (.clk(clk), .rst_n(rst_n));
  sub_mod u_named (.clk(clk), .rst_n(rst_n), .in(a), .out(b));
  sub_mod u_ordered (clk, rst_n, a, b);
  sub_mod #(.W(8)) u_wild (.*);
  sub_mod u_arr [0:1] (.clk(clk), .rst_n(rst_n), .in(a), .out(b));
  and g1 (b[0], a[0], a[1]);
endmodule
"#,
    ),
  ])?;
  let output = Command::new(cargo_bin!())
    .current_dir(dir.path())
    .args(["scan", "-r", "rule.yml", "--json"])
    .assert()
    .success()
    .get_output()
    .stdout
    .clone();
  let json: Value = from_slice(&output)?;
  let matches = json.as_array().expect("should be array");
  assert_eq!(matches.len(), 5, "should match 5 instantiation variants");
  Ok(())
}

#[test]
fn test_sg_scan_systemverilog_instantiation_extension_inference() -> Result<()> {
  let dir = create_test_files([
    ("rule.yml", SV_INST_RULE),
    (
      "a.v",
      "module sub_mod(input logic a, output logic b); endmodule module top(input logic a, output logic b); sub_mod u0(a, b); endmodule",
    ),
    (
      "b.svh",
      "module sub_mod #(parameter int W = 1)(input logic a, output logic b); endmodule module top(input logic a, output logic b); sub_mod #(.W(1)) u1(.*); endmodule",
    ),
    ("c.ts", "console.log(123)"),
  ])?;
  Command::new(cargo_bin!())
    .current_dir(dir.path())
    .args(["scan", "-r", "rule.yml"])
    .assert()
    .success()
    .stdout(contains("sub_mod u0(a, b);"))
    .stdout(contains("sub_mod #(.W(1)) u1(.*);"))
    .stdout(contains("console.log(123)").not());
  Ok(())
}

const MAX_DIAG_RULE: &str = "
id: max-result-rule
message: test rule
severity: warning
language: TypeScript
rule: { pattern: Some($A) }
";

#[test]
fn test_max_diagnostics_shown() -> Result<()> {
  // Create 4 files, each with one match
  let dir = create_test_files([
    ("sgconfig.yml", CONFIG),
    ("rules/rule.yml", MAX_DIAG_RULE),
    ("a.ts", "Some(1)"),
    ("b.ts", "Some(2)"),
    ("c.ts", "Some(3)"),
    ("d.ts", "Some(4)"),
  ])?;
  // With --max-results=2, should only output 2 matches
  let output = Command::new(cargo_bin!())
    .current_dir(dir.path())
    .args(["scan", "--json", "--max-results=2"])
    .assert()
    .success()
    .get_output()
    .stdout
    .clone();
  let json: Value = from_slice(&output)?;
  let matches = json.as_array().expect("should be array");
  assert_eq!(matches.len(), 2, "should output exactly 2 matches");
  Ok(())
}

#[test]
fn test_max_diagnostics_shown_single_file() -> Result<()> {
  // Single file with 4 matches
  let dir = create_test_files([
    ("sgconfig.yml", CONFIG),
    ("rules/rule.yml", MAX_DIAG_RULE),
    ("test.ts", "Some(1); Some(2); Some(3); Some(4)"),
  ])?;
  // With --max-results=2, should only output 2 matches
  let output = Command::new(cargo_bin!())
    .current_dir(dir.path())
    .args(["scan", "--json", "--max-results=2"])
    .assert()
    .success()
    .get_output()
    .stdout
    .clone();
  let json: Value = from_slice(&output)?;
  let matches = json.as_array().expect("should be array");
  assert_eq!(
    matches.len(),
    2,
    "should output exactly 2 matches from single file"
  );
  Ok(())
}

#[test]
fn test_max_diagnostics_shown_stdin() -> Result<()> {
  // Test --max-results with stdin input
  let input = "Some(1); Some(2); Some(3); Some(4)";
  let output = Command::new(cargo_bin!())
    .args([
      "scan",
      "--stdin",
      "--json",
      "--inline-rules",
      MAX_DIAG_RULE,
      "--max-results=2",
    ])
    .write_stdin(input)
    .assert()
    .success()
    .get_output()
    .stdout
    .clone();
  let json: Value = from_slice(&output)?;
  let matches = json.as_array().expect("should be array");
  assert_eq!(
    matches.len(),
    2,
    "should output exactly 2 matches from stdin"
  );
  Ok(())
}

#[test]
fn test_yaml_sgconfig_extension() -> Result<()> {
  let dir = create_test_files([("sgconfig.yaml", CONFIG), ("rules/rule.yml", FILE_RULE)])?;
  Command::new(cargo_bin!())
    .current_dir(dir.path())
    .args(["scan"])
    .assert()
    .success();
  Ok(())
}

#[test]
fn test_sg_scan_sarif_output() -> Result<()> {
  let dir = setup()?;
  Command::new(cargo_bin!())
    .current_dir(dir.path())
    .args(["scan", "--format", "sarif"])
    .assert()
    .success()
    .stdout(contains("\"version\""))
    .stdout(contains("\"runs\""))
    .stdout(contains("\"results\""))
    .stdout(contains("\"ruleId\": \"on-rule\""))
    .stdout(predicate::function(|output: &str| {
      // Verify it's valid JSON
      from_slice::<Value>(output.as_bytes()).is_ok()
    }));
  Ok(())
}

#[test]
fn test_sg_scan_systemverilog_sarif_output() -> Result<()> {
  let dir = create_test_files([
    ("rule.yml", SV_RULE),
    (
      "test.sv",
      "module m; initial begin $display(data); end endmodule",
    ),
  ])?;
  Command::new(cargo_bin!())
    .current_dir(dir.path())
    .args(["scan", "-r", "rule.yml", "--format", "sarif"])
    .assert()
    .success()
    .stdout(contains("\"ruleId\": \"sv-display\""))
    .stdout(contains("\"uri\": \"test.sv\""))
    .stdout(contains("\"text\": \"$display(data);\""))
    .stdout(predicate::function(|output: &str| {
      from_slice::<Value>(output.as_bytes()).is_ok()
    }));
  Ok(())
}

#[test]
fn test_sg_scan_sarif_with_fixes() -> Result<()> {
  let rule = "
id: use-let
message: Use let instead of var
severity: error
language: JavaScript
rule:
  pattern: var $VAR = $VAL
fix: let $VAR = $VAL
";
  let dir = create_test_files([("rule.yml", rule), ("test.js", "var x = 123;")])?;
  Command::new(cargo_bin!())
    .current_dir(dir.path())
    .args(["scan", "-r", "rule.yml", "--format", "sarif"])
    .assert()
    .stdout(contains("\"fixes\""))
    .stdout(contains("\"artifactChanges\""))
    .stdout(contains("\"replacements\""))
    .stdout(contains("\"deletedRegion\""))
    .stdout(contains("\"insertedContent\""))
    .stdout(predicate::function(|output: &str| {
      from_slice::<Value>(output.as_bytes()).is_ok()
    }));
  Ok(())
}

#[test]
fn test_status_code_success_with_no_match() -> Result<()> {
  let dir = create_test_files([("rule.yml", RULE1)])?;
  Command::new(cargo_bin!())
    .current_dir(dir.path())
    .args(["scan", "-r", "rule.yml"])
    .assert()
    .stdout(predicate::str::is_empty())
    .success();
  Ok(())
}
