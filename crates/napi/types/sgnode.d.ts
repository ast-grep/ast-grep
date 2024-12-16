import type {
  FieldNames,
  TypesInField,
  NodeTypesMap,
  ExtractField,
  NodeKinds,
  NodeFieldInfo,
  RootKind,
} from './staticTypes'
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
  T extends NodeKinds<M> = NodeKinds<M>,
> {
  range(): Range
  isLeaf(): boolean
  isNamed(): boolean
  isNamedLeaf(): boolean
  /** Returns the string name of the node kind */
  kind(): T
  /** Check if the node is the same kind as the given `kind` string */
  is<K extends T>(kind: K): this is SgNode<M, K>
  text(): string
  matches(m: string): boolean
  inside(m: string): boolean
  has(m: string): boolean
  precedes(m: string): boolean
  follows(m: string): boolean
  getMatch<K extends NodeKinds<M>>(m: string): SgNode<M, K> | null
  getMultipleMatches(m: string): Array<SgNode<M>>
  getTransformed(m: string): string | null
  /** Returns the node's SgRoot */
  getRoot(): SgRoot<M>
  children(): Array<SgNode<M>>
  /** Returns the node's id */
  id(): number
  find<K extends NodeKinds<M>>(
    matcher: string | number | NapiConfig<M>,
  ): SgNode<M, K> | null
  findAll<K extends NodeKinds<M>>(
    matcher: string | number | NapiConfig<M>,
  ): Array<SgNode<M, K>>
  /** Finds the first child node in the `field` */
  field<F extends FieldNames<M[T]>>(name: F): FieldSgNode<M, T, F>
  /** Finds all the children nodes in the `field` */
  fieldChildren<F extends FieldNames<M[T]>>(
    name: F,
  ): NonNullable<FieldSgNode<M, T, F>>[]
  parent<K extends NodeKinds<M>>(): SgNode<M, K> | null
  child<K extends NodeKinds<M>>(nth: number): SgNode<M, K> | null
  ancestors(): Array<SgNode<M>>
  next<K extends NodeKinds<M>>(): SgNode<M, K> | null
  nextAll(): Array<SgNode<M>>
  prev<K extends NodeKinds<M>>(): SgNode<M, K> | null
  prevAll(): Array<SgNode<M>>
  replace(text: string): Edit
  commitEdits(edits: Array<Edit>): string
}
/** Represents the parsed tree of code. */
export declare class SgRoot<M extends NodeTypesMap = NodeTypesMap> {
  /** Returns the root SgNode of the ast-grep instance. */
  root<K extends NodeKinds<M> = RootKind<M>>(): SgNode<M, K>
  /**
   * Returns the path of the file if it is discovered by ast-grep's `findInFiles`.
   * Returns `"anonymous"` if the instance is created by `lang.parse(source)`.
   */
  filename(): string
}

type FieldSgNode<
  M extends NodeTypesMap,
  K extends NodeKinds<M>,
  F extends FieldNames<M[K]>,
  I extends NodeFieldInfo = ExtractField<M[K], F>,
> = I extends { required: true }
  ? SgNode<M, TypesInField<M, I>>
  : SgNode<M, TypesInField<M, I>> | null