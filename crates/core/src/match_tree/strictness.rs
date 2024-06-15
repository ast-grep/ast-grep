enum MatchStrictness {
  Cst,         // all nodes are matched
  Smart,       // all nodes except source trivial nodes are matched.
  Significant, // only significant nodes are matched
  Ast,         // only ast nodes are matched
  Lenient,     // ast-nodes excluding comments are matched
}
