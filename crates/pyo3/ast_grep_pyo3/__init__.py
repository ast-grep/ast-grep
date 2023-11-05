from __future__ import annotations

from typing import List, TypedDict,  Literal, Dict, Union
from .ast_grep_pyo3 import SgNode, SgRoot, Pos, Range

class Pattern(TypedDict):
    selector: str
    context: str


class RuleWithoutNot(TypedDict, total=False):
    # atomic rule
    pattern: str | Pattern
    kind: str
    regex: str

    # relational rule
    inside: "Relation"
    has: "Relation"
    precedes: "Relation"
    follows: "Relation"

    # composite rule
    all: List["Rule"]
    any: List["Rule"]
    # cannot add here due to reserved keyword
    # not: Rule
    matches: str

# workaround
# Python's keyword requires `not` be a special case
class Rule(RuleWithoutNot, TypedDict("Not", {"not": "Rule"}, total=False)):
    pass

# Relational Rule Related
StopBy = Union[Literal["neighbor"], Literal["end"], Rule]

class Relation(Rule, total=False):
    stopBy: StopBy
    field: str

class Config(TypedDict, total=False):
    rule: Rule
    constraints: Dict[str, Dict]
    utils: Dict[str, Rule]
    transform: Dict[str, Dict]

__all__ = [
    "Rule",
    "Config",
    "Relation",
    "Pattern",
    "SgNode",
    "SgRoot",
    "Pos",
    "Range",
]