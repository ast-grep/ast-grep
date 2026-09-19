#![allow(dead_code)] // the shared test harness has helpers this file does not use

//! A member that is itself a declaration (a nested type, class, interface, enum,
//! struct, object) keeps its own members. The nested declaration's members are
//! read with the enclosing scope, so rule data needs no extra bookkeeping, and a
//! rule that declares its own child scope overrides that scope for its subtree.

use ast_grep_language::SupportLang;

mod common;

const CPP_RULES: &str = include_str!("../src/default_rules/cpp.yml");
const PYTHON_RULES: &str = include_str!("../src/default_rules/python.yml");
const KOTLIN_RULES: &str = include_str!("../src/default_rules/kotlin.yml");
const RUBY_RULES: &str = include_str!("../src/default_rules/ruby.yml");

/// The shape the Java nested-type rules take: a nested type is a member of its
/// enclosing type, and nothing else is declared — the inherited scope already
/// knows how to read a class's methods.
const JAVA_NESTED_RULES: &str = r#"
id: java-class
language: Java
role: item
symbolType: class
rule:
  all:
    - kind: class_declaration
    - has:
        field: name
        pattern: $NAME
name: $NAME
---
id: java-nested-class
language: Java
role: member
parentRuleIds: [java-class]
symbolType: class
rule:
  all:
    - kind: class_declaration
    - has:
        field: name
        pattern: $NAME
name: $NAME
---
id: java-member-method
language: Java
role: member
parentRuleIds: [java-class]
symbolType: method
rule:
  all:
    - kind: method_declaration
    - has:
        field: name
        pattern: $NAME
name: $NAME
"#;

#[test]
fn java_nested_class_keeps_its_members() {
  common::assert_outline_snapshot(
    SupportLang::Java,
    JAVA_NESTED_RULES,
    r#"
class Outer {
  class Inner {
    class Innermost {
      void deepest() {}
    }

    void innerMethod() {}
  }

  void outerMethod() {}
}
"#,
    r#"
- Class item exported Outer
  - Class public Inner
    - Class public Innermost
      - Method public deepest
    - Method public innerMethod
  - Method public outerMethod
"#,
  );
}

/// A rule that declares its own child scope replaces the inherited one for that
/// container's subtree.
#[test]
fn a_declared_child_scope_overrides_the_inherited_one() {
  common::assert_outline_snapshot(
    SupportLang::Java,
    r#"
id: java-class
language: Java
role: item
symbolType: class
rule:
  all:
    - kind: class_declaration
    - has:
        field: name
        pattern: $NAME
name: $NAME
---
id: java-nested-class
language: Java
role: member
parentRuleIds: [java-class]
symbolType: class
rule:
  all:
    - kind: class_declaration
    - has:
        field: name
        pattern: $NAME
name: $NAME
---
id: java-outer-method
language: Java
role: member
parentRuleIds: [java-class]
symbolType: method
rule:
  all:
    - kind: method_declaration
    - has:
        field: name
        pattern: $NAME
name: $NAME
---
id: java-inner-method
language: Java
role: member
parentRuleIds: [java-nested-class]
symbolType: function
rule:
  all:
    - kind: method_declaration
    - has:
        field: name
        pattern: $NAME
name: $NAME
"#,
    r#"
class Outer {
  class Inner {
    void innerMethod() {}
  }

  void outerMethod() {}
}
"#,
    r#"
- Class item exported Outer
  - Class public Inner
    - Function public innerMethod
  - Method public outerMethod
"#,
  );
}

/// C++: `cpp-nested-class` is a member of its enclosing class; its fields are
/// read with the inherited scope.
#[test]
fn cpp_nested_class_keeps_its_members() {
  common::assert_outline_snapshot(
    SupportLang::Cpp,
    CPP_RULES,
    r#"
class Outer {
public:
  class Inner {
  public:
    int field;
  };
};
"#,
    r#"
- Class item exported Outer
  - Class private Inner
    - Field private field
"#,
  );
}

#[test]
fn python_nested_class_keeps_its_members() {
  common::assert_outline_snapshot(
    SupportLang::Python,
    PYTHON_RULES,
    r#"
class Outer:
    class Inner:
        value = 1

        def method(self):
            return 1

    def outer_method(self):
        return 2
"#,
    r#"
- Class item exported Outer
  - Class public Inner
    - Field public value
    - Method public method
  - Method public outer_method
"#,
  );
}

#[test]
fn kotlin_nested_class_keeps_its_members() {
  common::assert_outline_snapshot(
    SupportLang::Kotlin,
    KOTLIN_RULES,
    r#"
class Outer {
    class Inner {
        val value = 1
        fun method() {}
    }
    fun outerMethod() {}
}
"#,
    r#"
- Class item exported Outer
  - Class public Inner
    - Property public value
    - Method public method
  - Method public outerMethod
"#,
  );
}

/// Ruby models `class` inside `module` (`ruby-module-member-class`); a `class`
/// inside a `class` has no such rule today, so its methods stay on the outer
/// class. Recorded so the remaining rule-data gap stays visible.
#[test]
fn ruby_class_in_class_is_still_a_rule_gap() {
  common::assert_outline_snapshot(
    SupportLang::Ruby,
    RUBY_RULES,
    r#"
class Outer
  class Inner
    def method; end
  end
end
"#,
    r#"
- Class item exported Outer
  - Method public method
"#,
  );
}
