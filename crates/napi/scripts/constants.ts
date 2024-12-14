import { Lang } from "..";

export const languageNodeTypesTagVersionOverrides: Partial<
  Record<Lang, string>
> = {
  // The latest version is not tagged yet, so we have to use the latest available tag
  [Lang.Kotlin]: "0.3.8",
};

export const languagesCrateNames: Record<Lang, string> = {
  [Lang.JavaScript]: "tree-sitter-javascript",
  [Lang.TypeScript]: "tree-sitter-typescript",
  [Lang.Tsx]: "tree-sitter-typescript",
  [Lang.Java]: "tree-sitter-java",
  [Lang.Python]: "tree-sitter-python",
  [Lang.Rust]: "tree-sitter-rust",
  [Lang.C]: "tree-sitter-c",
  [Lang.Cpp]: "tree-sitter-cpp",
  [Lang.Go]: "tree-sitter-go",
  [Lang.Html]: "tree-sitter-html",
  [Lang.Css]: "tree-sitter-css",
  [Lang.Json]: "tree-sitter-json",
  [Lang.CSharp]: "tree-sitter-c-sharp",
  [Lang.Ruby]: "tree-sitter-ruby",
  [Lang.Php]: "tree-sitter-php",
  [Lang.Elixir]: "tree-sitter-elixir",
  [Lang.Kotlin]: "tree-sitter-kotlin",
  [Lang.Swift]: "tree-sitter-swift",
  [Lang.Haskell]: "tree-sitter-haskell",
  [Lang.Scala]: "tree-sitter-scala",
  [Lang.Lua]: "tree-sitter-lua",
  [Lang.Bash]: "tree-sitter-bash",
  [Lang.Yaml]: "tree-sitter-yaml",
  [Lang.Sql]: "tree-sitter-sql",
};

export const languagesNodeTypesUrls = {
  [Lang.JavaScript]:
    "https://raw.githubusercontent.com/tree-sitter/tree-sitter-javascript/refs/tags/{{TAG}}/src/node-types.json",
  [Lang.TypeScript]:
    "https://raw.githubusercontent.com/tree-sitter/tree-sitter-typescript/refs/tags/{{TAG}}/typescript/src/node-types.json",
  [Lang.Tsx]:
    "https://raw.githubusercontent.com/tree-sitter/tree-sitter-typescript/refs/tags/{{TAG}}/tsx/src/node-types.json",
  [Lang.Java]:
    "https://raw.githubusercontent.com/tree-sitter/tree-sitter-java/refs/tags/{{TAG}}/src/node-types.json",
  [Lang.Python]:
    "https://raw.githubusercontent.com/tree-sitter/tree-sitter-python/refs/tags/{{TAG}}/src/node-types.json",
  [Lang.Rust]:
    "https://raw.githubusercontent.com/tree-sitter/tree-sitter-rust/refs/tags/{{TAG}}/src/node-types.json",
  [Lang.C]:
    "https://raw.githubusercontent.com/tree-sitter/tree-sitter-c/refs/tags/{{TAG}}/src/node-types.json",
  [Lang.Cpp]:
    "https://raw.githubusercontent.com/tree-sitter/tree-sitter-cpp/refs/tags/{{TAG}}/src/node-types.json",
  [Lang.Go]:
    "https://raw.githubusercontent.com/tree-sitter/tree-sitter-go/refs/tags/{{TAG}}/src/node-types.json",
  [Lang.Html]:
    "https://raw.githubusercontent.com/tree-sitter/tree-sitter-html/refs/tags/{{TAG}}/src/node-types.json",
  [Lang.Css]:
    "https://raw.githubusercontent.com/tree-sitter/tree-sitter-css/refs/tags/{{TAG}}/src/node-types.json",
  [Lang.Json]:
    "https://raw.githubusercontent.com/tree-sitter/tree-sitter-json/refs/tags/{{TAG}}/src/node-types.json",
  [Lang.CSharp]:
    "https://raw.githubusercontent.com/tree-sitter/tree-sitter-c-sharp/refs/tags/{{TAG}}/src/node-types.json",
  [Lang.Ruby]:
    "https://raw.githubusercontent.com/tree-sitter/tree-sitter-ruby/refs/tags/{{TAG}}/src/node-types.json",
  [Lang.Php]:
    "https://raw.githubusercontent.com/tree-sitter/tree-sitter-php/refs/tags/{{TAG}}/php/src/node-types.json",
  [Lang.Elixir]:
    "https://raw.githubusercontent.com/elixir-lang/tree-sitter-elixir/refs/tags/{{TAG}}/src/node-types.json",
  [Lang.Kotlin]:
    "https://raw.githubusercontent.com/fwcd/tree-sitter-kotlin/refs/tags/{{TAG}}/src/node-types.json",
  [Lang.Haskell]:
    "https://raw.githubusercontent.com/tree-sitter/tree-sitter-haskell/refs/tags/{{TAG}}/src/node-types.json",
  [Lang.Scala]:
    "https://raw.githubusercontent.com/tree-sitter/tree-sitter-scala/refs/tags/{{TAG}}/src/node-types.json",
  [Lang.Bash]:
    "https://raw.githubusercontent.com/tree-sitter/tree-sitter-bash/refs/tags/{{TAG}}/src/node-types.json",
  [Lang.Yaml]:
    "https://raw.githubusercontent.com/tree-sitter-grammars/tree-sitter-yaml/refs/tags/{{TAG}}/src/node-types.json",
  [Lang.Lua]:
    "https://raw.githubusercontent.com/tree-sitter-grammars/tree-sitter-lua/refs/tags/{{TAG}}/src/node-types.json",
  // Not available for SQL and Swift - They don't have node-types.json in their repo contents
};