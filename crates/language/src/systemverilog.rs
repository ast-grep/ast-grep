#![cfg(test)]
use super::*;

fn test_match(query: &str, source: &str) {
  use crate::test::test_match_lang;
  test_match_lang(query, source, SystemVerilog);
}

fn test_non_match(query: &str, source: &str) {
  use crate::test::test_non_match_lang;
  test_non_match_lang(query, source, SystemVerilog);
}

#[test]
fn test_systemverilog_pattern() {
  test_match(
    "module $M; $$$BODY endmodule",
    r#"
module m;
  logic a, b;
  assign a = b;
endmodule
"#,
  );
  test_match("assign $L = $R;", "assign a = b;");
  test_match(
    "always_comb begin $$$BODY end",
    "always_comb begin a = b; end",
  );
  test_match(
    "class $C; $$$MEMBERS endclass",
    "class Packet; rand bit [7:0] data; endclass",
  );
  test_match("$display($MSG);", "$display(data);");
  test_non_match("$display($MSG);", "$monitor(data);");
  test_non_match(
    "module n; $$$BODY endmodule",
    "module m; assign a = b; endmodule",
  );
}

#[test]
fn test_systemverilog_advanced_pattern() {
  test_match(
    "always_ff @($EVT) begin $$$BODY end",
    "always_ff @(posedge clk) begin a <= b; end",
  );
  test_match(
    "function void $F(); $$$BODY endfunction",
    "function void clear(); a = 0; endfunction",
  );
  test_match(
    "interface $I; $$$BODY endinterface",
    "interface bus_if; logic valid; endinterface",
  );
  test_match(
    "package $P; $$$BODY endpackage",
    "package util_pkg; typedef int data_t; endpackage",
  );
  test_match(
    "generate $$$BODY endgenerate",
    "generate if (1) begin logic x; end endgenerate",
  );
  test_match("assert property ($P);", "assert property (x == x);");
  test_non_match("assert property ($P);", "assume property (x == x);");
  test_match(
    "sub_mod #(.W(8)) $I($$$PORTS);",
    "sub_mod #(.W(8)) u_sub (.clk(clk), .rst_n(rst_n), .in(a), .out(b));",
  );
  test_match("sub_mod $I($$$PORTS);", "sub_mod u0 (clk, rst_n, a, b);");
  test_match("sub_mod #(.W(8)) $I(.*);", "sub_mod #(.W(8)) u1 (.*);");
  test_match(
    "sub_mod $I [0:1]($$$PORTS);",
    "sub_mod u_arr [0:1] (.clk(clk), .rst_n(rst_n), .in(a), .out(b));",
  );
  test_match(
    "axi_if #(32, 64) $I($$$PORTS);",
    "axi_if #(32, 64) m_if (.clk(clk), .rst_n(rst_n));",
  );
}

#[test]
fn test_systemverilog_preprocess() {
  assert_eq!(
    SystemVerilog.pre_process_pattern("assign $L = $R;"),
    "assign _L = _R;"
  );
  assert_eq!(
    SystemVerilog.pre_process_pattern("$display($MSG);"),
    "$display(_MSG);"
  );
  assert_eq!(
    SystemVerilog.pre_process_pattern("module $M; $$$BODY endmodule"),
    "module _M; ___BODY endmodule"
  );
}

fn test_replace(src: &str, pattern: &str, replacer: &str) -> String {
  use crate::test::test_replace_lang;
  test_replace_lang(src, pattern, replacer, SystemVerilog)
}

fn test_replace_all(src: &str, pattern: &str, replacer: &str) -> String {
  let mut source = SystemVerilog.ast_grep(src);
  while source
    .replace(pattern, replacer)
    .expect("should parse successfully")
  {}
  source.generate()
}

#[test]
fn test_systemverilog_replace() {
  let module_ret = test_replace(
    "module m; endmodule",
    "module $M; $$$BODY endmodule",
    "module top; endmodule",
  );
  assert_eq!(module_ret, "module top; endmodule");

  let assign_ret = test_replace("assign a = b;", "assign $L = $R;", "assign $L = c;");
  assert_eq!(assign_ret, "assign a = c;");

  let display_ret = test_replace("$display(data);", "$display($MSG);", "$monitor($MSG);");
  assert_eq!(display_ret, "$monitor(data);");

  let assert_ret = test_replace(
    "assert property (x == x);",
    "assert property ($P);",
    "assume property ($P);",
  );
  assert_eq!(assert_ret, "assume property (x == x);");
}

#[test]
fn test_systemverilog_replace_multi_match_stability() {
  let src = r#"
module m;
  initial begin
    $display(a); // keep trailing comment
    if (en) begin
      $display(b);
    end
  end
endmodule
"#;
  let ret = test_replace_all(src, "$display($MSG);", "$monitor($MSG);");
  assert_eq!(
    ret,
    r#"
module m;
  initial begin
    $monitor(a); // keep trailing comment
    if (en) begin
      $monitor(b);
    end
  end
endmodule
"#
  );
}

#[test]
fn test_systemverilog_replace_multiline_indent_stability() {
  let src = r#"
module m;
  always_comb begin
    a = b;
    c = d;
  end
endmodule
"#;
  let ret = test_replace(
    src,
    "always_comb begin $$$BODY end",
    r#"always_comb begin
  $display("trace");
  $$$BODY
end"#,
  );
  assert_eq!(
    ret,
    r#"
module m;
  always_comb begin
    $display("trace");
    a = b;
    c = d;
  end
endmodule
"#
  );
}

#[test]
fn test_systemverilog_fixture_regression() {
  let src = include_str!("../../../fixtures/systemverilog/uvm_tb_pkg.sv");
  test_match("package $P; $$$BODY endpackage", src);
  test_match("import $PKG::*;", src);
  test_match("class $C extends $B; $$$BODY endclass", src);
  test_match("task $T($$$ARGS); $$$BODY endtask", src);
  test_match("$display($MSG);", src);
}

#[test]
fn test_systemverilog_macro_preproc_boundary() {
  let src = r#"
`define SHOW(MSG) $display(MSG)
module m;
  initial begin
`ifdef ENABLE_LOG
    `SHOW(data);
`else
    $display("fallback");
`endif
  end
endmodule
"#;
  test_match("$display($MSG);", src);
  test_non_match("$display($MSG);", "`SHOW(data);");
}

#[test]
fn test_systemverilog_error_recovery_boundary() {
  let src = r#"
module m;
  initial begin
    if (en) $display(data)
  end
  assign a = b;
endmodule
"#;
  test_match("assign $L = $R;", src);
  test_non_match(
    "$display($MSG);",
    "module m; initial begin $display data; end endmodule",
  );
}
