use ast_grep_language::SupportLang;

#[allow(dead_code)]
mod common;

#[test]
fn csharp_rules_parse_and_extract_dotnet_shapes() {
  const RULES: &str = include_str!("../src/default_rules/csharp.yml");
  common::assert_outline_snapshot(
    SupportLang::CSharp,
    RULES,
    r#"
using System;
namespace Demo.Core;
public interface IService { void Run(); }
public class Parser { private int count; public Parser(int count) { this.count = count; } public string Parse(string input) { return input; } }
public enum Mode { Fast, Slow }
"#,
    r#"
- Module import private System
- Interface item exported IService
  - Method public Run
- Class item exported Parser
  - Field private count
  - Constructor public Parser
  - Method public Parse
- Enum item exported Mode
  - EnumMember public Fast
  - EnumMember public Slow
"#,
  );
}

#[test]
fn c_rules_parse_and_extract_native_shapes() {
  const RULES: &str = include_str!("../src/default_rules/c.yml");
  common::assert_outline_snapshot(
    SupportLang::C,
    RULES,
    r#"
#include <stdio.h>
typedef struct Config { int value; } Config;
enum Mode { Fast, Slow };
int count;
int helper(int value) { return value; }
"#,
    r#"
- Module import private <stdio.h>
- Struct item exported Config
  - Field public value
- Enum item exported Mode
  - EnumMember public Fast
  - EnumMember public Slow
- Variable item exported count
- Function item exported helper
"#,
  );
}

#[test]
fn cpp_rules_parse_and_extract_native_shapes() {
  const RULES: &str = include_str!("../src/default_rules/cpp.yml");
  common::assert_outline_snapshot(
    SupportLang::Cpp,
    RULES,
    r#"
#include <vector>
namespace demo {
class Parser { public: Parser(); int parse(const char* input); private: int count; };
struct Config { int value; };
enum Mode { Fast, Slow };
int helper(int value) { return value; }
}
"#,
    r#"
- Module import private <vector>
- Class item exported Parser
  - Constructor private Parser
  - Method private parse
  - Field private count
- Struct item exported Config
  - Field private value
- Enum item exported Mode
  - EnumMember public Fast
  - EnumMember public Slow
- Function item exported helper
"#,
  );
}
