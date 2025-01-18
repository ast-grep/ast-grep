import type { SgNode, SgRoot } from './sgnode'
import type { NapiConfig, FindConfig, FileOption } from './config'
import type { NapiLang, LanguageNodeTypes } from './lang'
import type { NamedKinds, TypesMap } from './staticTypes'

export declare function parseFiles<M extends TypesMap>(
  paths: Array<string> | FileOption,
  callback: (err: null | Error, result: SgRoot<M>) => void,
): Promise<number>
/** Parse a string to an ast-grep instance */
export declare function parse<M extends TypesMap, L extends NapiLang>(
  lang: L,
  src: string,
): SgRoot<L extends keyof LanguageNodeTypes ? LanguageNodeTypes[L] : M>
/**
 * Parse a string to an ast-grep instance asynchronously in threads.
 * It utilize multiple CPU cores when **concurrent processing sources**.
 * However, spawning excessive many threads may backfire.
 * Please refer to libuv doc, nodejs' underlying runtime
 * for its default behavior and performance tuning tricks.
 */
export declare function parseAsync<M extends TypesMap, L extends NapiLang>(
  lang: L,
  src: string,
): Promise<SgRoot<L extends keyof LanguageNodeTypes ? LanguageNodeTypes[L] : M>>
/** Get the `kind` number from its string name. */
export declare function kind<M extends TypesMap, L extends NapiLang>(
  lang: L,
  kindName: NamedKinds<
    L extends keyof LanguageNodeTypes ? LanguageNodeTypes[L] : M
  >,
): number
/** Compile a string to ast-grep Pattern. */
export declare function pattern<M extends TypesMap, L extends NapiLang>(
  lang: L,
  pattern: string,
): Promise<
  NapiConfig<L extends keyof LanguageNodeTypes ? LanguageNodeTypes[L] : M>
>
/**
 * Discover and parse multiple files in Rust.
 * `lang` specifies the language.
 * `config` specifies the file path and matcher.
 * `callback` will receive matching nodes found in a file.
 */
export declare function findInFiles<M extends TypesMap, L extends NapiLang>(
  lang: L,
  config: FindConfig<
    L extends keyof LanguageNodeTypes ? LanguageNodeTypes[L] : M
  >,
  callback: (
    err: null | Error,
    result: SgNode<
      L extends keyof LanguageNodeTypes ? LanguageNodeTypes[L] : M
    >[],
  ) => void,
): Promise<number>
