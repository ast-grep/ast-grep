/* tslint:disable */
/* eslint-disable */

/* auto-generated by NAPI-RS */

/**
 * Rule configuration similar to YAML
 * See https://ast-grep.github.io/reference/yaml.html
 */
export interface NapiConfig {
  /** The rule object, see https://ast-grep.github.io/reference/rule.html */
  rule: any
  /** See https://ast-grep.github.io/guide/rule-config.html#constraints */
  constraints?: any
  /** Available languages: html, css, js, jsx, ts, tsx */
  language?: FrontEndLanguage
  /** https://ast-grep.github.io/reference/yaml.html#transform */
  transform?: any
  /** https://ast-grep.github.io/guide/rule-config/utility-rule.html */
  utils?: any
}
export const enum FrontEndLanguage {
  Html = 'Html',
  JavaScript = 'JavaScript',
  Tsx = 'Tsx',
  Css = 'Css',
  TypeScript = 'TypeScript',
  Bash = 'Bash',
  C = 'C',
  Cpp = 'Cpp',
  CSharp = 'CSharp',
  Dart = 'Dart',
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
  Swift = 'Swift'
}
export interface FileOption {
  paths: Array<string>
  languageGlobs: Record<string, Array<string>>
}
export function parseFiles(paths: Array<string> | FileOption, callback: (err: null | Error, result: SgRoot) => void): Promise<number>
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
export interface Edit {
  /** The position of the edit */
  position: number
  /** The length of the text to be deleted */
  deletedLength: number
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
export function parse(lang: FrontEndLanguage, src: string): SgRoot
/**
 * Parse a string to an ast-grep instance asynchronously in threads.
 * It utilize multiple CPU cores when **concurrent processing sources**.
 * However, spawning excessive many threads may backfire.
 * Please refer to libuv doc, nodejs' underlying runtime
 * for its default behavior and performance tuning tricks.
 */
export function parseAsync(lang: FrontEndLanguage, src: string): Promise<SgRoot>
/** Get the `kind` number from its string name. */
export function kind(lang: FrontEndLanguage, kindName: string): number
/** Compile a string to ast-grep Pattern. */
export function pattern(lang: FrontEndLanguage, pattern: string): NapiConfig
/**
 * Discover and parse multiple files in Rust.
 * `lang` specifies the language.
 * `config` specifies the file path and matcher.
 * `callback` will receive matching nodes found in a file.
 */
export function findInFiles(lang: FrontEndLanguage, config: FindConfig, callback: (err: null | Error, result: SgNode[]) => void): Promise<number>
export class SgNode {
  range(): Range
  isLeaf(): boolean
  isNamed(): boolean
  isNamedLeaf(): boolean
  /** Returns the string name of the node kind */
  kind(): string
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
  children(): Array<SgNode>
  find(matcher: string | number | NapiConfig): SgNode | null
  findAll(matcher: string | number | NapiConfig): Array<SgNode>
  /** Finds the child node in the `field` */
  field(name: string): SgNode | null
  parent(): SgNode | null
  child(nth: number): SgNode | null
  ancestors(): Array<SgNode>
  next(): SgNode | null
  nextAll(): Array<SgNode>
  prev(): SgNode | null
  prevAll(): Array<SgNode>
  replace(text: string): Edit
  commitEdits(edits: Array<Edit>): string
}
/** Represents the parsed tree of code. */
export class SgRoot {
  /** Returns the root SgNode of the ast-grep instance. */
  root(): SgNode
  /**
   * Returns the path of the file if it is discovered by ast-grep's `findInFiles`.
   * Returns `"anonymous"` if the instance is created by `lang.parse(source)`.
   */
  filename(): string
}
export namespace html {
  /** Parse a string to an ast-grep instance */
  export function parse(src: string): SgRoot
  /**
   * Parse a string to an ast-grep instance asynchronously in threads.
   * It utilize multiple CPU cores when **concurrent processing sources**.
   * However, spawning excessive many threads may backfire.
   * Please refer to libuv doc, nodejs' underlying runtime
   * for its default behavior and performance tuning tricks.
   */
  export function parseAsync(src: string): Promise<SgRoot>
  /** Get the `kind` number from its string name. */
  export function kind(kindName: string): number
  /** Compile a string to ast-grep Pattern. */
  export function pattern(pattern: string): NapiConfig
  /**
   * Discover and parse multiple files in Rust.
   * `config` specifies the file path and matcher.
   * `callback` will receive matching nodes found in a file.
   */
  export function findInFiles(config: FindConfig, callback: (err: null | Error, result: SgNode[]) => void): Promise<number>
}
export namespace js {
  /** Parse a string to an ast-grep instance */
  export function parse(src: string): SgRoot
  /**
   * Parse a string to an ast-grep instance asynchronously in threads.
   * It utilize multiple CPU cores when **concurrent processing sources**.
   * However, spawning excessive many threads may backfire.
   * Please refer to libuv doc, nodejs' underlying runtime
   * for its default behavior and performance tuning tricks.
   */
  export function parseAsync(src: string): Promise<SgRoot>
  /** Get the `kind` number from its string name. */
  export function kind(kindName: string): number
  /** Compile a string to ast-grep Pattern. */
  export function pattern(pattern: string): NapiConfig
  /**
   * Discover and parse multiple files in Rust.
   * `config` specifies the file path and matcher.
   * `callback` will receive matching nodes found in a file.
   */
  export function findInFiles(config: FindConfig, callback: (err: null | Error, result: SgNode[]) => void): Promise<number>
}
export namespace jsx {
  /** Parse a string to an ast-grep instance */
  export function parse(src: string): SgRoot
  /**
   * Parse a string to an ast-grep instance asynchronously in threads.
   * It utilize multiple CPU cores when **concurrent processing sources**.
   * However, spawning excessive many threads may backfire.
   * Please refer to libuv doc, nodejs' underlying runtime
   * for its default behavior and performance tuning tricks.
   */
  export function parseAsync(src: string): Promise<SgRoot>
  /** Get the `kind` number from its string name. */
  export function kind(kindName: string): number
  /** Compile a string to ast-grep Pattern. */
  export function pattern(pattern: string): NapiConfig
  /**
   * Discover and parse multiple files in Rust.
   * `config` specifies the file path and matcher.
   * `callback` will receive matching nodes found in a file.
   */
  export function findInFiles(config: FindConfig, callback: (err: null | Error, result: SgNode[]) => void): Promise<number>
}
export namespace ts {
  /** Parse a string to an ast-grep instance */
  export function parse(src: string): SgRoot
  /**
   * Parse a string to an ast-grep instance asynchronously in threads.
   * It utilize multiple CPU cores when **concurrent processing sources**.
   * However, spawning excessive many threads may backfire.
   * Please refer to libuv doc, nodejs' underlying runtime
   * for its default behavior and performance tuning tricks.
   */
  export function parseAsync(src: string): Promise<SgRoot>
  /** Get the `kind` number from its string name. */
  export function kind(kindName: string): number
  /** Compile a string to ast-grep Pattern. */
  export function pattern(pattern: string): NapiConfig
  /**
   * Discover and parse multiple files in Rust.
   * `config` specifies the file path and matcher.
   * `callback` will receive matching nodes found in a file.
   */
  export function findInFiles(config: FindConfig, callback: (err: null | Error, result: SgNode[]) => void): Promise<number>
}
export namespace tsx {
  /** Parse a string to an ast-grep instance */
  export function parse(src: string): SgRoot
  /**
   * Parse a string to an ast-grep instance asynchronously in threads.
   * It utilize multiple CPU cores when **concurrent processing sources**.
   * However, spawning excessive many threads may backfire.
   * Please refer to libuv doc, nodejs' underlying runtime
   * for its default behavior and performance tuning tricks.
   */
  export function parseAsync(src: string): Promise<SgRoot>
  /** Get the `kind` number from its string name. */
  export function kind(kindName: string): number
  /** Compile a string to ast-grep Pattern. */
  export function pattern(pattern: string): NapiConfig
  /**
   * Discover and parse multiple files in Rust.
   * `config` specifies the file path and matcher.
   * `callback` will receive matching nodes found in a file.
   */
  export function findInFiles(config: FindConfig, callback: (err: null | Error, result: SgNode[]) => void): Promise<number>
}
export namespace css {
  /** Parse a string to an ast-grep instance */
  export function parse(src: string): SgRoot
  /**
   * Parse a string to an ast-grep instance asynchronously in threads.
   * It utilize multiple CPU cores when **concurrent processing sources**.
   * However, spawning excessive many threads may backfire.
   * Please refer to libuv doc, nodejs' underlying runtime
   * for its default behavior and performance tuning tricks.
   */
  export function parseAsync(src: string): Promise<SgRoot>
  /** Get the `kind` number from its string name. */
  export function kind(kindName: string): number
  /** Compile a string to ast-grep Pattern. */
  export function pattern(pattern: string): NapiConfig
  /**
   * Discover and parse multiple files in Rust.
   * `config` specifies the file path and matcher.
   * `callback` will receive matching nodes found in a file.
   */
  export function findInFiles(config: FindConfig, callback: (err: null | Error, result: SgNode[]) => void): Promise<number>
}
