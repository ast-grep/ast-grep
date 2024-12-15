import type { Rule } from './types/rule'
import {SgRoot, SgNode} from './types/sgnode'

//-----Type Only Export!-----//
// Just export Rule here and user can use Rule['pattern'] to get the type of pattern
export type { Rule } from './types/rule'
export type { Pos, Edit, Range } from './types/sgnode'

//-----Runtime Value Export!-----//
export { SgRoot, SgNode }
// deprecated
export * from './types/deprecated'

/**
 * Rule configuration similar to YAML
 * See https://ast-grep.github.io/reference/yaml.html
 */
export interface NapiConfig {
  /** The rule object, see https://ast-grep.github.io/reference/rule.html */
  rule: Rule
  /** See https://ast-grep.github.io/guide/rule-config.html#constraints */
  constraints?: Record<string, Rule>
  /** Available languages: html, css, js, jsx, ts, tsx */
  language?: Lang
  /**
   * transform is NOT useful in JavaScript. You can use JS code to directly transform the result.
   * https://ast-grep.github.io/reference/yaml.html#transform
   */
  transform?: unknown
  /** https://ast-grep.github.io/guide/rule-config/utility-rule.html */
  utils?: Record<string, Rule>
}
export interface FileOption {
  paths: Array<string>
  languageGlobs: Record<string, Array<string>>
}
export interface FindConfig {
  /** specify the file paths to recursively find files */
  paths: Array<string>
  /** a Rule object to find what nodes will match */
  matcher: NapiConfig
  /**
   * An list of pattern globs to treat of certain files in the specified language.
   * eg. ['*.vue', '*.svelte'] for html.findFiles, or ['*.ts'] for tsx.findFiles.
   * It is slightly different from https://ast-grep.github.io/reference/sgconfig.html#languageglobs
   */
  languageGlobs?: Array<string>
}

export declare function parseFiles(
  paths: Array<string> | FileOption,
  callback: (err: null | Error, result: SgRoot) => void,
): Promise<number>

export enum Lang {
  Html = 'Html',
  JavaScript = 'JavaScript',
  Tsx = 'Tsx',
  Css = 'Css',
  TypeScript = 'TypeScript',
  Bash = 'Bash',
  C = 'C',
  Cpp = 'Cpp',
  CSharp = 'CSharp',
  Go = 'Go',
  Elixir = 'Elixir',
  Haskell = 'Haskell',
  Java = 'Java',
  Json = 'Json',
  Kotlin = 'Kotlin',
  Lua = 'Lua',
  Php = 'Php',
  Python = 'Python',
  Ruby = 'Ruby',
  Rust = 'Rust',
  Scala = 'Scala',
  Sql = 'Sql',
  Swift = 'Swift',
  Yaml = 'Yaml',
}
/** Parse a string to an ast-grep instance */
export declare function parse(lang: Lang, src: string): SgRoot
/**
 * Parse a string to an ast-grep instance asynchronously in threads.
 * It utilize multiple CPU cores when **concurrent processing sources**.
 * However, spawning excessive many threads may backfire.
 * Please refer to libuv doc, nodejs' underlying runtime
 * for its default behavior and performance tuning tricks.
 */
export declare function parseAsync(lang: Lang, src: string): Promise<SgRoot>
/** Get the `kind` number from its string name. */
export declare function kind(lang: Lang, kindName: string): number
/** Compile a string to ast-grep Pattern. */
export declare function pattern(lang: Lang, pattern: string): NapiConfig
/**
 * Discover and parse multiple files in Rust.
 * `lang` specifies the language.
 * `config` specifies the file path and matcher.
 * `callback` will receive matching nodes found in a file.
 */
export declare function findInFiles(
  lang: Lang,
  config: FindConfig,
  callback: (err: null | Error, result: SgNode[]) => void,
): Promise<number>