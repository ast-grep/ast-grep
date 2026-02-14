package tb_pkg;
  import uvm_pkg::*;

  class base_test extends uvm_component;
    `uvm_component_utils(base_test)

    function new(string name, uvm_component parent);
      super.new(name, parent);
    endfunction

    task run_phase(uvm_phase phase);
      phase.raise_objection(this);
      $display("run");
      phase.drop_objection(this);
    endtask
  endclass
endpackage
