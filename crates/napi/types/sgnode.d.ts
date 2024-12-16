import type { FieldNames, GetSafeFieldType, NodeTypesMap, FieldTypeMeta, NodeKinds } from './staticTypes'
import type { NapiConfig } from './config'

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

export declare class SgNode<
  M extends NodeTypesMap = NodeTypesMap,
  T extends string = keyof M & string,
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

type FieldSgNode<
  Map extends NodeTypesMap,
  K extends NodeKinds<Map>,
  F extends FieldNames<Map[K]>,
  M extends FieldTypeMeta<Map[K], F> = FieldTypeMeta<Map[K], F>,
> = M['required'] extends true
  ? SgNode<Map, GetSafeFieldType<Map, K, F>>
  : SgNode<Map, GetSafeFieldType<Map, K, F>> | null