mod common;

use std::process::ExitCode;

use anyhow::Result;
use ast_grep::main_with_args;
use common::create_test_files;
use tempfile::TempDir;

const CONFIG: &str = "
ruleDirs:
- rules
testConfigs:
- testDir: rule-tests
";
const RULE: &str = "
id: test-rule
message: test rule
severity: warning
language: TypeScript
rule:
  pattern: Some($A)
";
const OFF_RULE: &str = "
id: test-rule
message: test rule
severity: off
language: TypeScript
rule:
  pattern: Some($A)
";

const TEST: &str = "
id: test-rule
valid:
- None
invalid:
- Some(123)
";

const WRONG_TEST: &str = "
id: test-rule
valid:
- Some(123)
invalid:
- None
";

fn setup() -> Result<TempDir> {
  let dir = create_test_files([
    ("sgconfig.yml", CONFIG),
    ("rules/test-rule.yml", RULE),
    ("rule-tests/test-rule-test.yml", TEST),
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
fn test_sg_test() -> Result<()> {
  let dir = setup()?;
  let config = dir.path().join("sgconfig.yml");
  let ret = sg(&format!(
    "ast-grep test -c {} --skip-snapshot-tests",
    config.display()
  ));
  assert!(ret.is_ok());
  drop(dir);
  Ok(())
}

fn setup_error() -> Result<TempDir> {
  let dir = create_test_files([
    ("sgconfig.yml", CONFIG),
    ("rules/test-rule.yml", RULE),
    ("rule-tests/test-rule-test.yml", WRONG_TEST),
    ("test.ts", "Some(123)"),
  ])?;
  assert!(dir.path().join("sgconfig.yml").exists());
  Ok(dir)
}

#[test]
fn test_sg_test_error() -> Result<()> {
  let dir = setup_error()?;
  let config = dir.path().join("sgconfig.yml");
  let ret = sg(&format!(
    "ast-grep test -c {} --skip-snapshot-tests",
    config.display()
  ));
  assert!(ret.is_err());
  drop(dir);
  Ok(())
}

// should skip/pick wrong_test based on filter
#[test]
fn test_sg_test_filter() -> Result<()> {
  let dir = setup_error()?;
  let config = dir.path().join("sgconfig.yml");
  let ret = sg(&format!(
    "ast-grep test -c {} --skip-snapshot-tests -f error-rule",
    config.display()
  ));
  assert!(ret.is_err());
  let ret = sg(&format!(
    "ast-grep test -c {} --skip-snapshot-tests -f test-rule",
    config.display()
  ));
  assert!(ret.is_err());
  drop(dir);
  Ok(())
}

#[test]
fn test_sg_test_off_rule() -> Result<()> {
  let dir = create_test_files([
    ("sgconfig.yml", CONFIG),
    ("rules/test-rule.yml", OFF_RULE),
    ("rule-tests/test-rule-test.yml", WRONG_TEST),
    ("test.ts", "Some(123)"),
  ])?;
  let config = dir.path().join("sgconfig.yml");
  let ret = sg(&format!(
    "ast-grep test -c {} --skip-snapshot-tests",
    config.display()
  ));
  assert!(ret.is_ok());
  let ret = sg(&format!(
    "ast-grep test -c {} --skip-snapshot-tests --include-off",
    config.display()
  ));
  assert!(ret.is_err());
  drop(dir);
  Ok(())
}

const SV_RULE: &str = "
id: sv-rule
message: sv test rule
severity: warning
language: systemverilog
rule:
  pattern: $display($A);
";

const SV_TEST: &str = "
id: sv-rule
valid:
- $monitor(data);
invalid:
- $display(data);
";

const SV_ALIAS_RULE: &str = "
id: sv-alias-rule
message: sv alias rule
severity: warning
language: sv
rule:
  pattern: $display($A);
";

const SV_ALIAS_TEST: &str = "
id: sv-alias-rule
valid:
- '`SHOW(data);'
invalid:
- $display(data);
- |
  module m;
    initial begin
      $display(data);
    end
";

const SV_INST_RULE: &str = "
id: sv-inst-rule
message: sv instantiation rule
severity: warning
language: systemverilog
rule:
  kind: module_instantiation
";

const SV_INST_TEST: &str = "
id: sv-inst-rule
valid:
- |
  module top(input logic [1:0] a, output logic y);
    and g1(y, a[0], a[1]);
  endmodule
invalid:
- |
  interface axi_if #(int W = 8) (input logic clk, rst_n);
  endinterface

  module sub_mod #(parameter int W = 8) (input logic clk, rst_n);
  endmodule

  module top(input logic clk, rst_n, input logic [7:0] a, output logic [7:0] b);
    axi_if #(8) m_if (.clk(clk), .rst_n(rst_n));
    sub_mod u0 (.clk(clk), .rst_n(rst_n), .in(a), .out(b));
    sub_mod #(.W(8)) u1 (.clk(clk), .rst_n(rst_n), .in(a), .out(b));
  endmodule
- |
  module sub_mod #(parameter int W = 8) (
    input logic clk,
    input logic rst_n,
    input logic [W-1:0] in,
    output logic [W-1:0] out
  );
  endmodule

  module top(input logic clk, rst_n, input logic [7:0] a, output logic [7:0] b);
    sub_mod u_ordered (clk, rst_n, a, b);
    sub_mod #(.W(8)) u_wild (.*);
    sub_mod u_arr [0:1] (.clk(clk), .rst_n(rst_n), .in(a), .out(b));
  endmodule
";

#[test]
fn test_sg_test_systemverilog() -> Result<()> {
  let dir = create_test_files([
    ("sgconfig.yml", CONFIG),
    ("rules/sv-rule.yml", SV_RULE),
    ("rule-tests/sv-rule-test.yml", SV_TEST),
  ])?;
  let config = dir.path().join("sgconfig.yml");
  let ret = sg(&format!(
    "ast-grep test -c {} --skip-snapshot-tests",
    config.display()
  ));
  assert!(ret.is_ok());
  Ok(())
}

#[test]
fn test_sg_test_systemverilog_alias_and_recovery() -> Result<()> {
  let dir = create_test_files([
    ("sgconfig.yml", CONFIG),
    ("rules/sv-alias-rule.yml", SV_ALIAS_RULE),
    ("rule-tests/sv-alias-rule-test.yml", SV_ALIAS_TEST),
  ])?;
  let config = dir.path().join("sgconfig.yml");
  let ret = sg(&format!(
    "ast-grep test -c {} --skip-snapshot-tests",
    config.display()
  ));
  assert!(ret.is_ok());
  Ok(())
}

#[test]
fn test_sg_test_systemverilog_instantiation() -> Result<()> {
  let dir = create_test_files([
    ("sgconfig.yml", CONFIG),
    ("rules/sv-inst-rule.yml", SV_INST_RULE),
    ("rule-tests/sv-inst-rule-test.yml", SV_INST_TEST),
  ])?;
  let config = dir.path().join("sgconfig.yml");
  let ret = sg(&format!(
    "ast-grep test -c {} --skip-snapshot-tests",
    config.display()
  ));
  assert!(ret.is_ok());
  Ok(())
}
