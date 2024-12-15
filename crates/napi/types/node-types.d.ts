import { SgNode } from "..";

export type NodeTypeSchema<
  ParentType extends string = string,
  FieldTypes extends string = string,
  ChildTypes extends string = string
> = {
  type: ParentType;
  named: boolean;
  root?: boolean;
  subtypes?: NodeTypeSchema<ParentType, FieldTypes, ChildTypes>[];
  fields?: {
    [key: string]: {
      multiple: boolean;
      required: boolean;
      types: ReadonlyArray<{ type: FieldTypes; named: boolean }>;
    };
  };
  children?: {
    multiple: boolean;
    required: boolean;
    types: ReadonlyArray<{ type: ChildTypes; named: boolean }>;
  };
};

export type NodeTypesMap = Record<string, NodeTypeSchema>;

export type FieldNames<N extends NodeTypeSchema> = N["fields"] extends Record<
  string,
  any
>
  ? keyof N["fields"]
  : string;

export type FieldTypeMeta<
  Map extends NodeTypeSchema,
  F extends FieldNames<Map>
> = Map["fields"] extends Record<
  string,
  { types: ReadonlyArray<{ type: string }> }
>
  ? Map["fields"][F]
  : {
      required: false;
      types: [{ type: string }];
    };

type GetSafeFieldType<
  Map extends NodeTypesMap,
  K extends keyof Map,
  F extends FieldNames<Map[K]>,
  M extends FieldTypeMeta<Map[K], F> = FieldTypeMeta<Map[K], F>
> = M["types"][number]["type"];

export type FieldSgNode<
  Map extends NodeTypesMap,
  K extends keyof Map,
  F extends FieldNames<Map[K]>,
  M extends FieldTypeMeta<Map[K], F> = FieldTypeMeta<Map[K], F>
> = M["required"] extends true
  ? SgNode<Map, GetSafeFieldType<Map, K, F>>
  : SgNode<Map, GetSafeFieldType<Map, K, F>> | null;