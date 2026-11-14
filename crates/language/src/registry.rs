// One record per language, in alphabetical enum and enumeration order.
// Registry and dispatch emitters consume the same normalized records.
macro_rules! with_languages {
  ($callback:ident $(, $argument:tt)*) => {
    normalize_languages! {
      $callback, ($($argument),*);
      Bash {
        feature: "tree-sitter-bash",
        parser: tree_sitter_bash::LANGUAGE,
        aliases: ["bash"],
        extensions: [
          "bash", "bats", "cgi", "command", "env", "fcgi", "ksh", "sh", "tmux", "tool", "zsh",
        ],
      },

      // https://en.cppreference.com/w/cpp/language/identifiers
      C {
        feature: "tree-sitter-c",
        parser: tree_sitter_c::LANGUAGE,
        aliases: ["c"],
        extensions: ["c", "h"],
        expando: '𐀀',
      },

      Cpp {
        feature: "tree-sitter-cpp",
        parser: tree_sitter_cpp::LANGUAGE,
        aliases: ["cc", "c++", "cpp", "cxx"],
        extensions: ["cc", "hpp", "cpp", "c++", "hh", "cxx", "cu", "ino"],
        expando: '𐀀',
      },

      // https://docs.microsoft.com/en-us/dotnet/csharp/language-reference/language-specification/lexical-structure#643-identifiers
      // all letter number is accepted
      // https://www.compart.com/en/unicode/category/Nl
      CSharp {
        feature: "tree-sitter-c-sharp",
        parser: tree_sitter_c_sharp::LANGUAGE,
        aliases: ["cs", "csharp"],
        extensions: ["cs"],
        expando: 'µ',
      },

      // https://www.w3.org/TR/CSS21/grammar.html#scanner
      Css {
        feature: "tree-sitter-css",
        parser: tree_sitter_css::LANGUAGE,
        aliases: ["css"],
        extensions: ["css", "scss"],
        expando: '_',
      },

      Dart {
        feature: "tree-sitter-dart",
        parser: tree_sitter_dart::LANGUAGE,
        aliases: ["dart"],
        extensions: ["dart"],
      },

      // https://github.com/elixir-lang/tree-sitter-elixir/blob/a2861e88a730287a60c11ea9299c033c7d076e30/grammar.js#L245
      Elixir {
        feature: "tree-sitter-elixir",
        parser: tree_sitter_elixir::LANGUAGE,
        aliases: ["ex", "elixir"],
        extensions: ["ex", "exs"],
        expando: 'µ',
      },

      // we can use any Unicode code point categorized as "Letter"
      // https://go.dev/ref/spec#letter
      Go {
        feature: "tree-sitter-go",
        parser: tree_sitter_go::LANGUAGE,
        aliases: ["go", "golang"],
        extensions: ["go"],
        expando: 'µ',
      },

      // GHC supports Unicode syntax per
      // https://ghc.gitlab.haskell.org/ghc/doc/users_guide/exts/unicode_syntax.html
      // and the tree-sitter-haskell grammar parses it too.
      Haskell {
        feature: "tree-sitter-haskell",
        parser: tree_sitter_haskell::LANGUAGE,
        aliases: ["hs", "haskell"],
        extensions: ["hs"],
        expando: 'µ',
      },

      // https://developer.hashicorp.com/terraform/language/syntax/configuration#identifiers
      Hcl {
        feature: "tree-sitter-hcl",
        parser: tree_sitter_hcl::LANGUAGE,
        aliases: ["hcl"],
        extensions: ["hcl", "nomad", "tf", "tfvars", "workflow"],
        expando: 'µ',
      },

      Html {
        feature: "tree-sitter-html",
        implementation: html::Html,
        aliases: ["html"],
        extensions: ["html", "htm", "xhtml"],
      },

      Java {
        feature: "tree-sitter-java",
        parser: tree_sitter_java::LANGUAGE,
        aliases: ["java"],
        extensions: ["java"],
      },

      JavaScript {
        feature: "tree-sitter-javascript",
        parser: tree_sitter_javascript::LANGUAGE,
        aliases: ["javascript", "js", "jsx"],
        extensions: ["cjs", "js", "mjs", "jsx"],
      },

      Json {
        feature: "tree-sitter-json",
        parser: tree_sitter_json::LANGUAGE,
        aliases: ["json"],
        extensions: ["json"],
      },

      // https://github.com/fwcd/tree-sitter-kotlin/pull/93
      Kotlin {
        feature: "tree-sitter-kotlin",
        parser: tree_sitter_kotlin::LANGUAGE,
        aliases: ["kotlin", "kt"],
        extensions: ["kt", "ktm", "kts"],
        expando: 'µ',
      },

      Lua {
        feature: "tree-sitter-lua",
        parser: tree_sitter_lua::LANGUAGE,
        aliases: ["lua"],
        extensions: ["lua"],
      },

      Markdown {
        feature: "tree-sitter-md",
        parser: tree_sitter_md::LANGUAGE,
        aliases: ["markdown", "md"],
        extensions: ["markdown", "md"],
      },

      // Nix uses $ for string interpolation (e.g., "${pkgs.hello}")
      Nix {
        feature: "tree-sitter-nix",
        parser: tree_sitter_nix::LANGUAGE,
        aliases: ["nix"],
        extensions: ["nix"],
        expando: '_',
      },

      // PHP accepts unicode to be used as some name not var name though
      Php {
        feature: "tree-sitter-php",
        parser: tree_sitter_php::LANGUAGE_PHP_ONLY,
        aliases: ["php"],
        extensions: ["php"],
        expando: 'µ',
      },

      // we can use any char in unicode range [:XID_Start:]
      // https://docs.python.org/3/reference/lexical_analysis.html#identifiers
      // see also [PEP 3131](https://peps.python.org/pep-3131/) for further details.
      Python {
        feature: "tree-sitter-python",
        parser: tree_sitter_python::LANGUAGE,
        aliases: ["py", "python"],
        extensions: ["py", "py3", "pyi", "bzl", "bazel"],
        expando: 'µ',
      },

      // https://github.com/tree-sitter/tree-sitter-ruby/blob/f257f3f57833d584050336921773738a3fd8ca22/grammar.js#L30C26-L30C78
      Ruby {
        feature: "tree-sitter-ruby",
        parser: tree_sitter_ruby::LANGUAGE,
        aliases: ["rb", "ruby"],
        extensions: ["rb", "rbw", "gemspec"],
        expando: 'µ',
      },

      // we can use any char in unicode range [:XID_Start:]
      // https://doc.rust-lang.org/reference/identifiers.html
      Rust {
        feature: "tree-sitter-rust",
        parser: tree_sitter_rust::LANGUAGE,
        aliases: ["rs", "rust"],
        extensions: ["rs"],
        expando: 'µ',
      },

      Scala {
        feature: "tree-sitter-scala",
        parser: tree_sitter_scala::LANGUAGE,
        aliases: ["scala"],
        extensions: ["scala", "sc", "sbt"],
      },

      Solidity {
        feature: "tree-sitter-solidity",
        parser: tree_sitter_solidity::LANGUAGE,
        aliases: ["sol", "solidity"],
        extensions: ["sol"],
      },

      //https://docs.swift.org/swift-book/documentation/the-swift-programming-language/lexicalstructure/#Identifiers
      Swift {
        feature: "tree-sitter-swift",
        parser: tree_sitter_swift::LANGUAGE,
        aliases: ["swift"],
        extensions: ["swift"],
        expando: 'µ',
      },

      Tsx {
        feature: "tree-sitter-typescript",
        parser: tree_sitter_typescript::LANGUAGE_TSX,
        aliases: ["tsx"],
        extensions: ["tsx"],
      },

      TypeScript {
        feature: "tree-sitter-typescript",
        parser: tree_sitter_typescript::LANGUAGE_TYPESCRIPT,
        aliases: ["ts", "typescript"],
        extensions: ["ts", "cts", "mts"],
      },

      Yaml {
        feature: "tree-sitter-yaml",
        parser: tree_sitter_yaml::LANGUAGE,
        aliases: ["yaml", "yml"],
        extensions: ["yaml", "yml"],
      },

      // Zig identifiers are [A-Za-z_][A-Za-z0-9_]*; '$' is not one of them.
      // https://ziglang.org/documentation/master/#Identifiers
      Zig {
        feature: "tree-sitter-zig",
        parser: tree_sitter_zig::LANGUAGE,
        aliases: ["zig"],
        extensions: ["zig"],
        expando: '_',
      },
    }
  };
}
