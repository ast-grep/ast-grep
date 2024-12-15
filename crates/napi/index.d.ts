import type { FieldNames, FieldSgNode, NodeTypesMap } from './types/node-types'
import type { Rule } from './types/rule'

// deprecated
export * from './types/deprecated'

// Just export Rule here and user can use Rule['pattern'] to get the type of pattern
export type { Rule } from './types/rule'

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
export declare function parseFiles(
  paths: Array<string> | FileOption,
  callback: (err: null | Error, result: SgRoot) => void,
): Promise<number>
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
export interface Edit {
  /** The start position of the edit */
  startPos: number
  /** The end position of the edit */
  endPos: number
  /** The text to be inserted */
  insertedText: string
}
export interface Pos {
  /** line number starting from 0 */
  line: number
  /** column number starting from 0 */
  column: number
  /** byte offset of the position */
  index: number
}
export interface Range {
  /** starting position of the range */
  start: Pos
  /** ending position of the range */
  end: Pos
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
export declare class SgNode<
  M extends NodeTypesMap = NodeTypesMap,
  T extends string = keyof M,
> {
  range(): Range
  isLeaf(): boolean
  isNamed(): boolean
  isNamedLeaf(): boolean
  /** Returns the string name of the node kind */
  kind(): T
  /** Check if the node is the same kind as the given `kind` string */
  is<K extends T>(kind: K): this is SgNode<M, K> & this
  text(): string
  matches(m: string): boolean
  inside(m: string): boolean
  has(m: string): boolean
  precedes(m: string): boolean
  follows(m: string): boolean
  getMatch(m: string): SgNode | null
  getMultipleMatches(m: string): Array<SgNode>
  getTransformed(m: string): string | null
  /** Returns the node's SgRoot */
  getRoot(): SgRoot
  children(): Array<SgNode<M>>
  /** Returns the node's id */
  id(): number
  find(matcher: string | number | NapiConfig): SgNode<M> | null
  findAll(matcher: string | number | NapiConfig): Array<SgNode<M>>
  /** Finds the first child node in the `field` */
  field<F extends FieldNames<M[T]>>(name: F): FieldSgNode<M, T, F>
  /** Finds all the children nodes in the `field` */
  fieldChildren<F extends FieldNames<M[T]>>(
    name: F,
  ): Exclude<FieldSgNode<M, T, F>, null>[]
  parent(): SgNode | null
  child(nth: number): SgNode<M> | null
  ancestors(): Array<SgNode>
  next(): SgNode | null
  nextAll(): Array<SgNode>
  prev(): SgNode | null
  prevAll(): Array<SgNode>
  replace(text: string): Edit
  commitEdits(edits: Array<Edit>): string
}
/** Represents the parsed tree of code. */
export declare class SgRoot<M extends NodeTypesMap = NodeTypesMap> {
  /** Returns the root SgNode of the ast-grep instance. */
  root(): SgNode<M>
  /**
   * Returns the path of the file if it is discovered by ast-grep's `findInFiles`.
   * Returns `"anonymous"` if the instance is created by `lang.parse(source)`.
   */
  filename(): string
}