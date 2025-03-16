export type WasmLang = "typescript" | "tsx";

export declare class SgNode<
  M extends TypesMap = TypesMap,
  out T extends Kinds<M> = Kinds<M>
> {
  /** Returns the node's id */
  id(): number;
  range(): Range;
  isLeaf(): boolean;
  isNamed(): boolean;
  isNamedLeaf(): boolean;
  text(): string;
  matches(m: string | number | WasmConfig<M>): boolean;
  inside(m: string | number | WasmConfig<M>): boolean;
  has(m: string | number | WasmConfig<M>): boolean;
  precedes(m: string | number | WasmConfig<M>): boolean;
  follows(m: string | number | WasmConfig<M>): boolean;
  /** Returns the string name of the node kind */
  kind(): T;
  readonly kindToRefine: T;
  /** Check if the node is the same kind as the given `kind` string */
  is<K extends T>(kind: K): this is SgNode<M, K>;
  // we need this override to allow string literal union
  is(kind: string): boolean;

  getMatch: NodeMethod<M, [mv: string]>;
  getMultipleMatches(m: string): Array<SgNode<M>>;
  getTransformed(m: string): string | null;
  /** Returns the node's SgRoot */
  getRoot(): SgRoot<M>;
  children(): Array<SgNode<M>>;
  find: NodeMethod<M, [matcher: string | number | WasmConfig<M>]>;
  findAll<K extends Kinds<M>>(
    matcher: string | number | WasmConfig<M>
  ): Array<RefineNode<M, K>>;
  /** Finds the first child node in the `field` */
  field<F extends FieldNames<M[T]>>(name: F): FieldNode<M, T, F>;
  /** Finds all the children nodes in the `field` */
  fieldChildren<F extends FieldNames<M[T]>>(
    name: F
  ): Exclude<FieldNode<M, T, F>, null>[];
  parent: NodeMethod<M>;
  child(nth: number): SgNode<M, ChildKinds<M, T>> | null;
  child<K extends NamedChildKinds<M, T>>(nth: number): RefineNode<M, K> | null;
  ancestors(): Array<SgNode<M>>;
  next: NodeMethod<M>;
  nextAll(): Array<SgNode<M>>;
  prev: NodeMethod<M>;
  prevAll(): Array<SgNode<M>>;
  replace(text: string): Edit;
  commitEdits(edits: Array<Edit>): string;
}
/** Represents the parsed tree of code. */
export declare class SgRoot<M extends TypesMap = TypesMap> {
  /** Returns the root SgNode of the ast-grep instance. */
  root(): SgNode<M, RootKind<M>>;
  /**
   * Returns the path of the file if it is discovered by ast-grep's `findInFiles`.
   * Returns `"anonymous"` if the instance is created by `lang.parse(source)`.
   */
  filename(): string;
}

interface NodeMethod<M extends TypesMap, Args extends unknown[] = []> {
  (...args: Args): SgNode<M> | null;
  <K extends NamedKinds<M>>(...args: Args): RefineNode<M, K> | null;
}

/**
 * if K contains string, return general SgNode. Otherwise,
 * if K is a literal union, return a union of SgNode of each kind.
 */
type RefineNode<M extends TypesMap, K> = string extends K
  ? SgNode<M>
  : K extends Kinds<M>
  ? SgNode<M, K>
  : never;

/**
 * return the SgNode of the field in the node.
 */
// F extends string is used to prevent noisy TS hover info
type FieldNode<
  M extends TypesMap,
  K extends Kinds<M>,
  F extends FieldNames<M[K]>
> = F extends string ? FieldNodeImpl<M, ExtractField<M[K], F>> : never;

type FieldNodeImpl<M extends TypesMap, I extends NodeFieldInfo> = I extends {
  required: true;
}
  ? RefineNode<M, TypesInField<M, I>>
  : RefineNode<M, TypesInField<M, I>> | null;

/**
 * Rule configuration similar to YAML
 * See https://ast-grep.github.io/reference/yaml.html
 */
export interface WasmConfig<M extends TypesMap = TypesMap> {
  /** The rule object, see https://ast-grep.github.io/reference/rule.html */
  rule: Rule<M>;
  /** See https://ast-grep.github.io/guide/rule-config.html#constraints */
  constraints?: Record<string, Rule<M>>;
  /** Builtin Language or custom language */
  language?: WasmLang;
  /**
   * transform is NOT useful in JavaScript. You can use JS code to directly transform the result.
   * https://ast-grep.github.io/reference/yaml.html#transform
   */
  transform?: unknown;
  /** https://ast-grep.github.io/guide/rule-config/utility-rule.html */
  utils?: Record<string, Rule<M>>;
}

export interface FileOption {
  paths: Array<string>;
  languageGlobs: Record<string, Array<string>>;
}

export interface FindConfig<M extends TypesMap = TypesMap> {
  /** specify the file paths to recursively find files */
  paths: Array<string>;
  /** a Rule object to find what nodes will match */
  matcher: WasmConfig<M>;
  /**
   * An list of pattern globs to treat of certain files in the specified language.
   * eg. ['*.vue', '*.svelte'] for html.findFiles, or ['*.ts'] for tsx.findFiles.
   * It is slightly different from https://ast-grep.github.io/reference/sgconfig.html#languageglobs
   */
  languageGlobs?: Array<string>;
}

export type Strictness = "cst" | "smart" | "ast" | "relaxed" | "signature";

export interface PatternObject<M extends TypesMap = TypesMap> {
  context: string;
  selector?: NamedKinds<M>; // only named node types
  strictness?: Strictness;
}

export type PatternStyle<M extends TypesMap = TypesMap> =
  | string
  | PatternObject<M>;

export interface Relation<M extends TypesMap = TypesMap> extends Rule<M> {
  /**
   * Specify how relational rule will stop relative to the target node.
   */
  stopBy?: "neighbor" | "end" | Rule<M>;
  /** Specify the tree-sitter field in parent node. Only available in has/inside rule. */
  field?: string;
}

export interface NthChildObject<M extends TypesMap = TypesMap> {
  /** The position in nodes' sibling list. It can be a number of An+B string */
  position: string | number;
  ofRule?: Rule<M>;
  reverse?: boolean;
}

/**
 * NthChild can have these types:
 * * number: the position of the node in the sibling list.
 * * string: An + B style string like CSS nth-child selector.
 * * object: An object with `position` and `ofRule` fields.
 */
export type NthChild<M extends TypesMap = TypesMap> =
  | number
  | string
  | NthChildObject<M>;

export interface Rule<M extends TypesMap = TypesMap> {
  /** A pattern string or a pattern object. */
  pattern?: PatternStyle<M>;
  /** The kind name of the node to match. You can look up code's kind names in playground. */
  kind?: NamedKinds<M>;
  /** The exact range of the node in the source code. */
  range?: Range;
  /** A Rust regular expression to match the node's text. https://docs.rs/regex/latest/regex/#syntax */
  regex?: string;
  /**
   * `nthChild` accepts number, string or object.
   * It specifies the position in nodes' sibling list. */
  nthChild?: NthChild<M>;

  // relational
  /**
   * `inside` accepts a relational rule object.
   * the target node must appear inside of another node matching the `inside` sub-rule. */
  inside?: Relation<M>;
  /**
   * `has` accepts a relational rule object.
   * the target node must has a descendant node matching the `has` sub-rule. */
  has?: Relation<M>;
  /**
   * `precedes` accepts a relational rule object.
   * the target node must appear before another node matching the `precedes` sub-rule. */
  precedes?: Relation<M>;
  /**
   * `follows` accepts a relational rule object.
   * the target node must appear after another node matching the `follows` sub-rule. */
  follows?: Relation<M>;
  // composite
  /**
   * A list of sub rules and matches a node if all of sub rules match.
   * The meta variables of the matched node contain all variables from the sub-rules. */
  all?: Array<Rule<M>>;
  /**
   * A list of sub rules and matches a node if any of sub rules match.
   * The meta variables of the matched node only contain those of the matched sub-rule. */
  any?: Array<Rule<M>>;
  /** A single sub-rule and matches a node if the sub rule does not match. */
  not?: Rule<M>;
  /** A utility rule id and matches a node if the utility rule matches. */
  matches?: string;
}

type Separator =
  | "caseChange"
  | "dash"
  | "dot"
  | "slash"
  | "space"
  | "underscore";

type StringCase =
  | "lowerCase"
  | "upperCase"
  | "capitalize"
  | "camelCase"
  | "snakeCase"
  | "kebabCase"
  | "pascalCase";

interface Substring {
  endChar?: number | null;
  source: string;
  startChar?: number | null;
}

interface Replace {
  by: string;
  replace: string;
  source: string;
}

type Transformation =
  | { substring: Substring }
  | { replace: Replace }
  | { convert: Convert }
  | { rewrite: Rewrite };

interface Rewrite {
  joinBy?: string | null;
  rewriters: string[];
  source: string;
}

interface SerializableFixConfig {
  expandEnd?: Relation;
  expandStart?: Relation;
  template: string;
}

type SerializableFixer = string | SerializableFixConfig;

export type Severity = "hint" | "info" | "warning" | "error" | "off";

export interface CompleteRuleConfig<M extends TypesMap = TypesMap>
  extends WasmConfig<M> {
  id: string;
  fix?: SerializableFixer;
  transform?: Record<string, Transformation>;
  severity?: Severity;
  note?: string;
  message?: string;
  ignores?: string[];
  url?: string;
}

// Definitions
interface Convert {
  separatedBy?: Separator[] | null;
  source: string;
  toCase: StringCase;
}

export interface Edit {
  /** The start position of the edit */
  startPos: number;
  /** The end position of the edit */
  endPos: number;
  /** The text to be inserted */
  insertedText: string;
}
export interface Position {
  /** line number starting from 0 */
  line: number;
  /** column number starting from 0 */
  column: number;
  /** byte offset of the position */
  index: number;
}
export interface Range {
  /** starting position of the range */
  start: Position;
  /** ending position of the range */
  end: Position;
}

/**
 * Reference
 * https://tree-sitter.github.io/tree-sitter/using-parsers#static-node-types
 * Rust CLI Impl
 * https://github.com/tree-sitter/tree-sitter/blob/f279d10aa2aca37c0004d84b2261685739f3cab8/cli/generate/src/node_types.rs#L35-L47
 */

export interface NodeBasicInfo {
  type: string;
  named: boolean;
}

export interface NodeFieldInfo {
  multiple: boolean;
  required: boolean;
  types: NodeBasicInfo[];
}

export interface NodeType extends NodeBasicInfo {
  root?: boolean;
  fields?: {
    [fieldName: string]: NodeFieldInfo;
  };
  children?: NodeFieldInfo;
  subtypes?: NodeBasicInfo[];
}

/**
 * A map of key to NodeType.
 * Note, the key is not necessary node's kind.
 * it can be a rule representing a category of syntax nodes
 * (e.g. “expression”, “type”, “declaration”).
 * See reference above for more details.
 */
export interface TypesMap {
  [key: string]: NodeType;
}

export type FieldNames<N extends NodeType> = N extends { fields: infer F }
  ? keyof F
  : string;

export type ExtractField<
  N extends NodeType,
  F extends FieldNames<N>
> = N["fields"] extends Record<F, NodeFieldInfo>
  ? N["fields"][F]
  : NodeFieldInfo;

// in case of empty types array, return string as fallback
type NoNever<T, Fallback> = [T] extends [never] ? Fallback : T;

export type TypesInField<M extends TypesMap, I extends NodeFieldInfo> = NoNever<
  ResolveType<M, I["types"][number]["type"]>,
  Kinds<M>
>;

export type NamedChildKinds<
  M extends TypesMap,
  T extends Kinds<M>
> = M[T] extends { children: infer C extends NodeFieldInfo }
  ? TypesInField<M, C>
  : NamedKinds<M>;
export type ChildKinds<M extends TypesMap, T extends Kinds<M>> =
  | NamedChildKinds<M, T>
  | LowPriorityKey;

/**
 * resolve subtypes alias. see tree-sitter's reference
 * e.g. like `expression` => `binary_expression` | `unary_expression` | ...
 */
type ResolveType<M extends TypesMap, K> = K extends keyof M
  ? M[K] extends { subtypes: infer S extends NodeBasicInfo[] }
    ? ResolveType<M, S[number]["type"]>
    : K
  : K;

/**
 * All named nodes' kinds that are usable in ast-grep rule
 * NOTE: SgNode can return kind not in this list
 */
export type NamedKinds<M extends TypesMap> = ResolveType<M, keyof M>;

/**
 * See open-ended unions / string literal completion in TypeScript
 * https://github.com/microsoft/TypeScript/issues/26277
 * https://github.com/microsoft/TypeScript/issues/33471
 */
type LowPriorityKey = string & {};

/**
 * A union of all named node kinds and a low priority key
 * tree-sitter Kinds also include unnamed nodes which is not usable in rule
 * NOTE: SgNode can return a string type if it is not a named node
 */
export type Kinds<M extends TypesMap = TypesMap> =
  | NamedKinds<M>
  | LowPriorityKey;

/**
 * The root node kind of the tree.
 */
export type RootKind<M extends TypesMap> = NoNever<
  Extract<M[keyof M], { root: true }>["type"],
  Kinds<M>
>;
