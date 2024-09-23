export type Strictness =
  | 'cst' | 'smart' | 'ast' | 'relaxed' | 'signature'

export interface PatternObject {
  context: string
  selector?: string
  strictness?: Strictness
}

export type PatternStyle = string | PatternObject

export interface Relation extends Rule {
  /**
   * Specify how relational rule will stop relative to the target node.
   */
  stopBy?: 'neighbor' | 'end' | Rule
  /** Specify the tree-sitter field in parent node. Only available in has/inside rule. */
  field?: string
}

export interface NthChildObject {
  /** The position in nodes' sibling list. It can be a number of An+B string */
  position: string | number
  ofRule?: Rule
  reverse?: boolean
}

/**
 * NthChild can have these types:
 * * number: the position of the node in the sibling list.
 * * string: An + B style string like CSS nth-child selector.
 * * object: An object with `position` and `ofRule` fields.
 */
export type NthChild = number | string | NthChildObject

export interface Rule {
  /** A pattern string or a pattern object. */
  pattern?: PatternStyle
  /** The kind name of the node to match. You can look up code's kind names in playground. */
  kind?: string
  /** A Rust regular expression to match the node's text. https://docs.rs/regex/latest/regex/#syntax */
  regex?: string
  /**
   * `nthChild` accepts number, string or object.
   * It specifies the position in nodes' sibling list. */
  nthChild?: NthChild

  // relational
  /**
   * `inside` accepts a relational rule object.
   * the target node must appear inside of another node matching the `inside` sub-rule. */
  inside?: Relation
  /**
   * `has` accepts a relational rule object.
   * the target node must has a descendant node matching the `has` sub-rule. */
  has?: Relation
  /**
   * `precedes` accepts a relational rule object.
   * the target node must appear before another node matching the `precedes` sub-rule. */
  precedes?: Relation
  /**
   * `follows` accepts a relational rule object.
   * the target node must appear after another node matching the `follows` sub-rule. */
  follows?: Relation
  // composite
  /**
   * A list of sub rules and matches a node if all of sub rules match.
   * The meta variables of the matched node contain all variables from the sub-rules. */
  all?: Array<Rule>
  /**
   * A list of sub rules and matches a node if any of sub rules match.
   * The meta variables of the matched node only contain those of the matched sub-rule. */
  any?: Array<Rule>
  /** A single sub-rule and matches a node if the sub rule does not match. */
  not?: Rule
  /** A utility rule id and matches a node if the utility rule matches. */
  matches?: string
}