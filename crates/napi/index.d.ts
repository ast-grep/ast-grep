import {SgRoot, SgNode} from './types/sgnode'
import {NapiConfig, FindConfig, FileOption} from './types/config'

//-----Type Only Export!-----//
// Only Rule here. User can use Rule['pattern'], e.g., to get the type of subfield.
export type { Rule } from './types/rule'
export type { Pos, Edit, Range } from './types/sgnode'
export type { NapiConfig, FindConfig, FileOption } from './types/config'

//-----Runtime Value Export!-----//
export { SgRoot, SgNode }
// deprecated
export * from './types/deprecated'

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