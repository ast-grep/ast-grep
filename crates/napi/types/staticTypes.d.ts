/**
 * Reference
 * https://tree-sitter.github.io/tree-sitter/using-parsers#static-node-types
 * Rust CLI Impl
 * https://github.com/tree-sitter/tree-sitter/blob/f279d10aa2aca37c0004d84b2261685739f3cab8/cli/generate/src/node_types.rs#L35-L47
 */

export interface NodeBasicInfo {
  type: string
  named: boolean
}

export interface NodeFieldInfo {
  multiple: boolean
  required: boolean
  types: NodeBasicInfo[]
}

export interface NodeType extends NodeBasicInfo {
  root?: boolean
  fields?: {
    [fieldName: string]: NodeFieldInfo
  }
  children?: NodeFieldInfo
  subtypes?: NodeBasicInfo[]
}

/**
 * A map of key to NodeType.
 * Note, the key is not necessary node's kind.
 * it can be a rule representing a category of syntax nodes
 * (e.g. “expression”, “type”, “declaration”).
 * See reference above for more details.
 */
export interface NodeTypesMap {
  [key: string]: NodeType
}

export type FieldNames<N extends NodeType> = N extends { fields: infer F }
  ? keyof F
  : string

export type ExtractField<
  N extends NodeType,
  F extends FieldNames<N>,
> = N['fields'] extends Record<F, NodeFieldInfo>
  ? N['fields'][F]
  : NodeFieldInfo

// in case of empty types array, return string as fallback
type NoNever<T, Fallback = string> = [T] extends [never] ? Fallback : T

export type TypesInField<
  M extends NodeTypesMap,
  I extends NodeFieldInfo,
> = NoNever<ResolveType<M, I['types'][number]['type']>>

// resolve subtypes alias
// e.g. like `expression` => `binary_expression` | `unary_expression` | ...
type ResolveType<M extends NodeTypesMap, K> = K extends keyof M
  ? M[K] extends { subtypes: infer S extends NodeBasicInfo[] }
    ? ResolveType<M, S[number]['type']>
    : K
  : K

type LowPriorityKey = string & {}

export type NodeKinds<M extends NodeTypesMap = NodeTypesMap> =
  | ResolveType<M, keyof M>
  | LowPriorityKey

export type RootKind<M extends NodeTypesMap> = NoNever<
  Extract<M[keyof M], { root: true }>['type'],
  NodeKinds<M>
>
