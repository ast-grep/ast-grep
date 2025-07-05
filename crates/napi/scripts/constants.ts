import { Lang } from '..'

export const languagesCrateNames: Record<Lang, string> = {
  [Lang.JavaScript]: 'tree-sitter-javascript',
  [Lang.TypeScript]: 'tree-sitter-typescript',
  [Lang.Tsx]: 'tree-sitter-typescript',
  [Lang.Html]: 'tree-sitter-html',
  [Lang.Css]: 'tree-sitter-css',
}

export const languagesNodeTypesUrls = {
  [Lang.JavaScript]:
    'https://raw.githubusercontent.com/tree-sitter/tree-sitter-javascript/refs/tags/{{TAG}}/src/node-types.json',
  [Lang.TypeScript]:
    'https://raw.githubusercontent.com/tree-sitter/tree-sitter-typescript/refs/tags/{{TAG}}/typescript/src/node-types.json',
  [Lang.Tsx]:
    'https://raw.githubusercontent.com/tree-sitter/tree-sitter-typescript/refs/tags/{{TAG}}/tsx/src/node-types.json',
  [Lang.Html]: 'https://raw.githubusercontent.com/tree-sitter/tree-sitter-html/refs/tags/{{TAG}}/src/node-types.json',
  [Lang.Css]: 'https://raw.githubusercontent.com/tree-sitter/tree-sitter-css/refs/tags/{{TAG}}/src/node-types.json',
}
