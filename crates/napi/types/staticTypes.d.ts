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
  types: readonly NodeBasicInfo[]
}

export interface NodeType extends NodeBasicInfo {
  root?: boolean
  fields?: {
    [fieldName: string]: NodeFieldInfo
  }
  children?: NodeFieldInfo
  subtypes?: readonly NodeBasicInfo[]
}

export interface NodeTypesMap {
  [key: string]: NodeType
}

export type FieldNames<N extends NodeType> =
  N extends { fields: infer F } ? keyof F : string

export type FieldTypeMeta<
  N extends NodeType,
  F extends FieldNames<N>,
> = N['fields'] extends Record<F, NodeFieldInfo>
  ? N['fields'][F]
  : NodeFieldInfo

export type GetSafeFieldType<
  M extends FieldTypeMeta<Map[K], F> = FieldTypeMeta<Map[K], F>,
> = M['types'][number]['type']

// TODO: this is wrong, we should resolve subtypes
export type NodeKinds<M extends NodeTypesMap>
  = keyof M & string