from typing import List, Optional, TypedDict, Unpack, Literal, overload, Dict

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
class Rule(RuleWithoutNot, TypedDict("Not", {"not": Rule}, total=False)):
    pass

# Relational Rule Related
StopBy = Literal["neighbor"] | Literal["end"] | Rule

class Relation(Rule, total=False):
    stop_by: StopBy
    field: str

class Config(TypedDict, total=False):
    rule: Rule
    constraints: Dict[str, Dict]
    utils: Dict[str, Rule]
    transform: Dict[str, Dict]

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
    def matches(self, **rule: Unpack[Rule]) -> bool: ...
    def inside(self, **rule: Unpack[Rule]) -> bool: ...
    def has(self, **rule: Unpack[Rule]) -> bool: ...
    def precedes(self, **rule: Unpack[Rule]) -> bool: ...
    def follows(self, **rule: Unpack[Rule]) -> bool: ...
    def get_match(self, meta_var: str) -> Optional[SgNode]: ...
    def get_multiple_matches(self, meta_var: str) -> List[SgNode]: ...
    def __getitem__(self, meta_var: str) -> SgNode: ...

    # Tree Traversal
    def get_root(self) -> SgRoot: ...
    @overload
    def find(self, config=None) -> Optional[SgNode]: ...
    @overload
    def find(self, **kwargs: Unpack[Rule]) -> Optional[SgNode]: ...
    @overload
    def find_all(self, config=None) -> List[SgNode]: ...
    @overload
    def find_all(self, **kwargs: Unpack[Rule]) -> List[SgNode]: ...
    def field(self, name: str) -> Optional[SgNode]: ...
    def parent(self) -> Optional[SgNode]: ...
    def child(self, nth: int) -> Optional[SgNode]: ...
    def children(self) -> List[SgNode]: ...
    def ancestors(self) -> List[SgNode]: ...
    def next(self) -> Optional[SgNode]: ...
    def next_all(self) -> List[SgNode]: ...
    def prev(self) -> Optional[SgNode]: ...
    def prev_all(self) -> List[SgNode]: ...

__all__ = [
    "Rule",
    "Config",
    "Pattern",
    "SgNode",
    "SgRoot",
    "Pos",
    "Range",
]