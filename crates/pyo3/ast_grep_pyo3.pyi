from typing import List, Optional, TypedDict, Unpack, Literal

class Pattern(TypedDict):
  selector: str
  context: str

class RuleWithoutNot(TypedDict, total=False):
  # atomic rule
  pattern: str | Pattern
  kind: str
  regex: str

  # relational rule
  inside: Relation
  has: Relation
  precedes: Relation
  follows: Relation

  # composite rule
  all: List[Rule]
  any: List[Rule]
  # TODO: make this better documented
  # not: Rule
  matches: str

# workaround
# Python's keyword requires `not` be a special case
class Rule(RuleWithoutNot, TypedDict('Not', {'not': Rule}, total=False)):
  pass

# Relational Rule Related
StopBy = Literal['neighbor'] | Literal['end'] | Rule

class Relation(Rule, total=False):
  stop_by: StopBy
  field: str

class Pos:
  line: int
  column: int
  index: int

class Range:
  start: Pos
  end: Pos

class SgRoot:
  def __init__(self, src: str, language: str) -> None: ...
  def root(self) -> SgNode: ...
  def filename(self) -> str: ...

class SgNode:
  # Node Inspection
  def range(self) -> Range: ...
  def is_leaf(self) -> bool: ...
  def is_named(self) -> bool: ...
  def is_named_leaf(self) -> bool: ...
  def kind(self) -> str: ...
  def text(self) -> str: ...

  # Search Refinement
  def matches(self, m: str) -> bool: ...
  def inside(self, m: str) -> bool: ...
  def has(self, m: str) -> bool: ...
  def precedes(self, m: str) -> bool: ...
  def follows(self, m: str) -> bool: ...
  def get_match(self, meta_var: str) -> Optional[SgNode]: ...
  def get_multiple_matches(self, meta_var: str) -> List[SgNode]: ...

  # Tree Traversal
  def get_root(self) -> SgRoot: ...
  def find(
    self,
    config = None,
    **kwargs: Unpack[Rule]
  ) -> SgNode: ...
  def find_all(
    self,
    config = None,
    **kwargs: Unpack[Rule]
  ) -> List[SgNode]: ...