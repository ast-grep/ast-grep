/*
Legacy reference from an earlier outline prototype.

PR 1 intentionally does not compile or ship builtin outline rules. Keep this
draft nearby as design input for the later extraction-rule and builtin-rule PRs.

pub const DEFAULT_OUTLINE_RULES: &str = r#"
extractors:
  - id: rust-use
    language: Rust
    kind: module
    role: import
    name: text
    exported: textPrefix:pub
    rule: { kind: use_declaration }
  - id: rust-mod
    language: Rust
    kind: module
    role: definition
    name: field:name
    exported: textPrefix:pub
    rule: { kind: mod_item }
  - id: rust-function
    language: Rust
    kind: function
    role: definition
    name: field:name
    exported: textPrefix:pub
    rule: { kind: function_item }
  - id: rust-struct
    language: Rust
    kind: struct
    role: definition
    name: field:name
    exported: textPrefix:pub
    rule: { kind: struct_item }
  - id: rust-enum
    language: Rust
    kind: enum
    role: definition
    name: field:name
    exported: textPrefix:pub
    rule: { kind: enum_item }
  - id: rust-trait
    language: Rust
    kind: interface
    role: definition
    name: field:name
    exported: textPrefix:pub
    rule: { kind: trait_item }
  - id: rust-type
    language: Rust
    kind: interface
    role: definition
    name: field:name
    exported: textPrefix:pub
    rule: { kind: type_item }
  - id: rust-const
    language: Rust
    kind: constant
    role: definition
    name: field:name
    exported: textPrefix:pub
    rule: { kind: const_item }
  - id: rust-static
    language: Rust
    kind: variable
    role: definition
    name: field:name
    exported: textPrefix:pub
    rule: { kind: static_item }
  - id: rust-impl
    language: Rust
    kind: object
    role: definition
    name: auto
    exported: never
    rule: { kind: impl_item }
  - id: rust-field
    language: Rust
    kind: field
    role: definition
    name: field:name
    exported: textPrefix:pub
    rule: { kind: field_declaration }
  - id: rust-enum-variant
    language: Rust
    kind: enumMember
    role: definition
    name: auto
    exported: never
    rule: { kind: enum_variant }

  - id: ts-import
    language: TypeScript
    kind: module
    role: import
    name: text
    exported: never
    rule: { kind: import_statement }
  - id: ts-re-export
    language: TypeScript
    kind: module
    role: export
    name: text
    exported: always
    rule:
      all:
        - kind: export_statement
        - regex: '^\s*export\s+(\{|\*|type\s+\{)'
  - id: ts-function
    language: TypeScript
    kind: function
    role: definition
    name: field:name
    exported: ancestorKind:export_statement
    rule: { kind: function_declaration }
  - id: ts-class
    language: TypeScript
    kind: class
    role: definition
    name: field:name
    exported: ancestorKind:export_statement
    rule:
      all:
        - kind: class_declaration
        - not:
            inside:
              kind: statement_block
              stopBy: end
  - id: ts-interface
    language: TypeScript
    kind: interface
    role: definition
    name: field:name
    exported: ancestorKind:export_statement
    rule: { kind: interface_declaration }
  - id: ts-type
    language: TypeScript
    kind: interface
    role: definition
    name: field:name
    exported: ancestorKind:export_statement
    rule: { kind: type_alias_declaration }
  - id: ts-method-signature
    language: TypeScript
    kind: method
    role: definition
    name: field:name
    exported: never
    rule:
      all:
        - kind: method_signature
        - not:
            inside:
              kind: statement_block
              stopBy: end
  - id: ts-property-signature
    language: TypeScript
    kind: field
    role: definition
    name: field:name
    exported: never
    rule:
      all:
        - kind: property_signature
        - not:
            inside:
              kind: statement_block
              stopBy: end
  - id: ts-method
    language: TypeScript
    kind: method
    role: definition
    name: field:name
    exported: never
    rule:
      all:
        - kind: method_definition
        - not:
            inside:
              kind: statement_block
              stopBy: end
  - id: ts-field
    language: TypeScript
    kind: field
    role: definition
    name: auto
    exported: never
    rule:
      all:
        - kind: public_field_definition
        - not:
            inside:
              kind: statement_block
              stopBy: end
  - id: ts-const
    language: TypeScript
    kind: constant
    role: definition
    name: auto
    exported: ancestorKind:export_statement
    rule:
      all:
        - kind: lexical_declaration
        - regex: '^\s*const\b'
        - not:
            inside:
              kind: statement_block
              stopBy: end
        - not:
            has:
              kind: arrow_function
              stopBy: end
  - id: ts-const-function
    language: TypeScript
    kind: function
    role: definition
    name: auto
    exported: ancestorKind:export_statement
    rule:
      all:
        - kind: lexical_declaration
        - regex: '^\s*const\b'
        - has:
            kind: arrow_function
            stopBy: end

  - id: tsx-import
    language: Tsx
    kind: module
    role: import
    name: text
    exported: never
    rule: { kind: import_statement }
  - id: tsx-re-export
    language: Tsx
    kind: module
    role: export
    name: text
    exported: always
    rule:
      all:
        - kind: export_statement
        - regex: '^\s*export\s+(\{|\*|type\s+\{)'
  - id: tsx-function
    language: Tsx
    kind: function
    role: definition
    name: field:name
    exported: ancestorKind:export_statement
    rule: { kind: function_declaration }
  - id: tsx-class
    language: Tsx
    kind: class
    role: definition
    name: field:name
    exported: ancestorKind:export_statement
    rule:
      all:
        - kind: class_declaration
        - not:
            inside:
              kind: statement_block
              stopBy: end
  - id: tsx-interface
    language: Tsx
    kind: interface
    role: definition
    name: field:name
    exported: ancestorKind:export_statement
    rule: { kind: interface_declaration }
  - id: tsx-type
    language: Tsx
    kind: interface
    role: definition
    name: field:name
    exported: ancestorKind:export_statement
    rule: { kind: type_alias_declaration }
  - id: tsx-method-signature
    language: Tsx
    kind: method
    role: definition
    name: field:name
    exported: never
    rule:
      all:
        - kind: method_signature
        - not:
            inside:
              kind: statement_block
              stopBy: end
  - id: tsx-property-signature
    language: Tsx
    kind: field
    role: definition
    name: field:name
    exported: never
    rule:
      all:
        - kind: property_signature
        - not:
            inside:
              kind: statement_block
              stopBy: end
  - id: tsx-method
    language: Tsx
    kind: method
    role: definition
    name: field:name
    exported: never
    rule:
      all:
        - kind: method_definition
        - not:
            inside:
              kind: statement_block
              stopBy: end
  - id: tsx-field
    language: Tsx
    kind: field
    role: definition
    name: auto
    exported: never
    rule:
      all:
        - kind: public_field_definition
        - not:
            inside:
              kind: statement_block
              stopBy: end
  - id: tsx-const
    language: Tsx
    kind: constant
    role: definition
    name: auto
    exported: ancestorKind:export_statement
    rule:
      all:
        - kind: lexical_declaration
        - regex: '^\s*const\b'
        - not:
            inside:
              kind: statement_block
              stopBy: end
        - not:
            has:
              kind: arrow_function
              stopBy: end
  - id: tsx-const-function
    language: Tsx
    kind: function
    role: definition
    name: auto
    exported: ancestorKind:export_statement
    rule:
      all:
        - kind: lexical_declaration
        - regex: '^\s*const\b'
        - has:
            kind: arrow_function
            stopBy: end

  - id: js-import
    language: JavaScript
    kind: module
    role: import
    name: text
    exported: never
    rule: { kind: import_statement }
  - id: js-re-export
    language: JavaScript
    kind: module
    role: export
    name: text
    exported: always
    rule:
      all:
        - kind: export_statement
        - regex: '^\s*export\s+(\{|\*)'
  - id: js-function
    language: JavaScript
    kind: function
    role: definition
    name: field:name
    exported: ancestorKind:export_statement
    rule: { kind: function_declaration }
  - id: js-class
    language: JavaScript
    kind: class
    role: definition
    name: field:name
    exported: ancestorKind:export_statement
    rule:
      all:
        - kind: class_declaration
        - not:
            inside:
              kind: statement_block
              stopBy: end
  - id: js-method
    language: JavaScript
    kind: method
    role: definition
    name: field:name
    exported: never
    rule:
      all:
        - kind: method_definition
        - not:
            inside:
              kind: statement_block
              stopBy: end
  - id: js-field
    language: JavaScript
    kind: field
    role: definition
    name: auto
    exported: never
    rule:
      all:
        - kind: public_field_definition
        - not:
            inside:
              kind: statement_block
              stopBy: end
  - id: js-const
    language: JavaScript
    kind: constant
    role: definition
    name: auto
    exported: ancestorKind:export_statement
    rule:
      all:
        - kind: lexical_declaration
        - regex: '^\s*const\b'
        - not:
            inside:
              kind: statement_block
              stopBy: end
        - not:
            has:
              kind: arrow_function
              stopBy: end
  - id: js-const-function
    language: JavaScript
    kind: function
    role: definition
    name: auto
    exported: ancestorKind:export_statement
    rule:
      all:
        - kind: lexical_declaration
        - regex: '^\s*const\b'
        - has:
            kind: arrow_function
            stopBy: end

  - id: py-import
    language: Python
    kind: module
    role: import
    name: text
    exported: never
    rule: { kind: import_statement }
  - id: py-from-import
    language: Python
    kind: module
    role: import
    name: text
    exported: never
    rule: { kind: import_from_statement }
  - id: py-function
    language: Python
    kind: function
    role: definition
    name: field:name
    exported: never
    rule: { kind: function_definition }
  - id: py-class
    language: Python
    kind: class
    role: definition
    name: field:name
    exported: never
    rule: { kind: class_definition }

  - id: go-package
    language: Go
    kind: package
    role: definition
    name: auto
    exported: never
    rule: { kind: package_clause }
  - id: go-import
    language: Go
    kind: module
    role: import
    name: text
    exported: never
    rule: { kind: import_declaration }
  - id: go-function
    language: Go
    kind: function
    role: definition
    name: field:name
    exported: nameUppercase
    rule: { kind: function_declaration }
  - id: go-method
    language: Go
    kind: method
    role: definition
    name: field:name
    exported: nameUppercase
    rule: { kind: method_declaration }
  - id: go-type
    language: Go
    kind: interface
    role: definition
    name: auto
    exported: nameUppercase
    rule: { kind: type_declaration }
  - id: go-const
    language: Go
    kind: constant
    role: definition
    name: auto
    exported: nameUppercase
    rule: { kind: const_declaration }
  - id: go-var
    language: Go
    kind: variable
    role: definition
    name: auto
    exported: nameUppercase
    rule: { kind: var_declaration }

  - id: java-package
    language: Java
    kind: package
    role: definition
    name: text
    exported: never
    rule: { kind: package_declaration }
  - id: java-import
    language: Java
    kind: module
    role: import
    name: text
    exported: never
    rule: { kind: import_declaration }
  - id: java-class
    language: Java
    kind: class
    role: definition
    name: field:name
    exported: textPrefix:public
    rule: { kind: class_declaration }
  - id: java-record
    language: Java
    kind: struct
    role: definition
    name: field:name
    exported: textPrefix:public
    rule: { kind: record_declaration }
  - id: java-interface
    language: Java
    kind: interface
    role: definition
    name: field:name
    exported: textPrefix:public
    rule: { kind: interface_declaration }
  - id: java-annotation
    language: Java
    kind: interface
    role: definition
    name: field:name
    exported: textPrefix:public
    rule: { kind: annotation_type_declaration }
  - id: java-enum
    language: Java
    kind: enum
    role: definition
    name: field:name
    exported: textPrefix:public
    rule: { kind: enum_declaration }
  - id: java-method
    language: Java
    kind: method
    role: definition
    name: field:name
    exported: textPrefix:public
    rule: { kind: method_declaration }
  - id: java-constructor
    language: Java
    kind: constructor
    role: definition
    name: field:name
    exported: textPrefix:public
    rule: { kind: constructor_declaration }
  - id: java-public-static-final-constant
    language: Java
    kind: constant
    role: definition
    name: $NAME
    exported: always
    rule:
      pattern:
        context: class A { public static final $T $NAME = $V; }
        selector: field_declaration

  - id: kotlin-import
    language: Kotlin
    kind: module
    role: import
    name: text
    exported: never
    rule: { kind: import_header }
  - id: kotlin-class
    language: Kotlin
    kind: class
    role: definition
    name: auto
    exported: notTextPrefixAny:private,internal
    rule:
      all:
        - kind: class_declaration
        - regex: '^\s*(public\s+)?class\b'
  - id: kotlin-interface
    language: Kotlin
    kind: interface
    role: definition
    name: auto
    exported: notTextPrefixAny:private,internal
    rule:
      all:
        - kind: class_declaration
        - regex: '^\s*(public\s+)?interface\b'
  - id: kotlin-object
    language: Kotlin
    kind: object
    role: definition
    name: auto
    exported: notTextPrefixAny:private,internal
    rule: { kind: object_declaration }
  - id: kotlin-function
    language: Kotlin
    kind: function
    role: definition
    name: auto
    exported: notTextPrefixAny:private,internal
    rule:
      all:
        - kind: function_declaration
        - not:
            inside:
              kind: function_body
              stopBy: end
  - id: kotlin-property
    language: Kotlin
    kind: variable
    role: definition
    name: auto
    exported: notTextPrefixAny:private,internal
    rule:
      all:
        - kind: property_declaration
        - not:
            inside:
              kind: function_body
              stopBy: end
  - id: kotlin-typealias
    language: Kotlin
    kind: interface
    role: definition
    name: auto
    exported: notTextPrefixAny:private,internal
    rule: { kind: type_alias }

  - id: swift-import
    language: Swift
    kind: module
    role: import
    name: text
    exported: never
    rule: { kind: import_declaration }
  - id: swift-class
    language: Swift
    kind: class
    role: definition
    name: firstNameLike
    exported: textPrefixAny:public,open
    rule:
      any:
        - all:
            - kind: class_declaration
            - regex: '^\s*(public\s+|open\s+)?class\b'
        - all:
            - kind: function_declaration
            - regex: '^\s*(public\s+|open\s+)?class\b'
  - id: swift-struct
    language: Swift
    kind: struct
    role: definition
    name: firstNameLike
    exported: textPrefix:public
    rule:
      all:
        - kind: class_declaration
        - regex: '^\s*(public\s+)?struct\b'
  - id: swift-enum
    language: Swift
    kind: enum
    role: definition
    name: firstNameLike
    exported: textPrefix:public
    rule:
      all:
        - kind: class_declaration
        - regex: '^\s*(public\s+)?enum\b'
  - id: swift-protocol
    language: Swift
    kind: interface
    role: definition
    name: field:name
    exported: textPrefixAny:public,open
    rule: { kind: protocol_declaration }
  - id: swift-function
    language: Swift
    kind: function
    role: definition
    name: field:name
    exported: textPrefixAny:public,open
    rule:
      all:
        - kind: function_declaration
        - regex: '^\s*(@[A-Za-z_][A-Za-z0-9_]*(\([^)]*\))?\s+)*(public\s+|open\s+|internal\s+|fileprivate\s+|private\s+)?(static\s+|class\s+|mutating\s+|nonmutating\s+|override\s+|final\s+)*func\b'
        - not:
            inside:
              kind: function_body
              stopBy: end
  - id: swift-property
    language: Swift
    kind: variable
    role: definition
    name: field:name
    exported: textPrefixAny:public,open
    rule:
      all:
        - kind: property_declaration
        - not:
            inside:
              kind: function_body
              stopBy: end
  - id: swift-typealias
    language: Swift
    kind: interface
    role: definition
    name: field:name
    exported: textPrefixAny:public,open
    rule: { kind: typealias_declaration }
"#;
*/
