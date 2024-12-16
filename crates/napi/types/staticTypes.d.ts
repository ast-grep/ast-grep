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

export type FieldNames<N extends NodeType> = N['fields'] extends Record<
  string,
  unknown
>
  ? keyof N['fields']
  : string

export type FieldTypeMeta<
  Map extends NodeType,
  F extends FieldNames<Map>,
> = Map['fields'] extends Record<
  string,
  { types: ReadonlyArray<{ type: string }> }
>
  ? Map['fields'][F]
  : {
      required: false
      types: [{ type: string }]
    }

export type GetSafeFieldType<
  Map extends NodeTypesMap,
  K extends keyof Map,
  F extends FieldNames<Map[K]>,
  M extends FieldTypeMeta<Map[K], F> = FieldTypeMeta<Map[K], F>,
> = M['types'][number]['type']