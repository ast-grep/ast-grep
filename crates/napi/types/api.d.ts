import type { SgNode, SgRoot } from './sgnode'
import type { NapiConfig, FindConfig, FileOption } from './config'
import type { Lang } from './lang'

export declare function parseFiles(
  paths: Array<string> | FileOption,
  callback: (err: null | Error, result: SgRoot) => void,
): Promise<number>
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
