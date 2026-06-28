use ast_grep_language::SupportLang;

#[allow(dead_code)]
mod common;

#[test]
fn ruby_rules_parse_and_extract_module_shapes() {
  const RULES: &str = include_str!("../src/default_rules/ruby.yml");
  common::assert_outline_snapshot(
    SupportLang::Ruby,
    RULES,
    r#"
require "json"
module Demo
  class Parser
    CONSTANT = 1
    def initialize(count)
      @count = count
    end
    def parse(input)
      input
    end
  end
end
def helper(value)
  value
end
"#,
    r#"
- Module import private "json"
- Module item exported Demo
  - Class public Parser
- Function item exported helper
"#,
  );
}

#[test]
fn php_rules_parse_and_extract_web_shapes() {
  const RULES: &str = include_str!("../src/default_rules/php.yml");
  common::assert_outline_snapshot(
    SupportLang::Php,
    RULES,
    r#"
<?php
namespace Demo;
use Vendor\Package as Package;
interface Service { public function run(): void; }
class Parser { private int $count; public function __construct(int $count) { $this->count = $count; } public function parse(string $input): string { return $input; } }
enum Mode { case Fast; case Slow; }
function helper(int $value): int { return $value; }
"#,
    r#"
- Module import private Vendor\Package
- Interface item exported Service
  - Method public run
- Class item exported Parser
  - Property private $count
  - Method public __construct
  - Method public parse
- Enum item exported Mode
  - EnumMember public Fast
  - EnumMember public Slow
- Function item exported helper
"#,
  );
}
