from __future__ import annotations

from typing import List, TypedDict,  Literal, Dict, Union, Mapping
from .ast_grep_py import SgNode, SgRoot, Pos, Range

class Pattern(TypedDict):
    selector: str
    context: str

class RuleWithoutNot(TypedDict, total=False):
    # atomic rule
    pattern: str | Pattern
    kind: str
    regex: str

    # relational rule
    inside: "Relation" # pyright report error if forward reference here?
    has: Relation
    precedes: Relation
    follows: Relation

    # composite rule
    all: List[Rule]
    any: List[Rule]
    # cannot add here due to reserved keyword
    # not: Rule
    matches: str

# workaround
# Python's keyword requires `not` be a special case
class Rule(RuleWithoutNot, TypedDict("Not", {"not": "Rule"}, total=False)):
    pass

# Relational Rule Related
StopBy = Union[Literal["neighbor"], Literal["end"], Rule]

# Relation do NOT inherit from Rule due to pyright bug
# see tests/test_rule.py
class Relation(RuleWithoutNot, TypedDict("Not", {"not": "Rule"}, total=False), total=False):
    stopBy: StopBy
    field: str

class Config(TypedDict, total=False):
    rule: Rule
    constraints: Dict[str, Mapping]
    utils: Dict[str, Rule]
    transform: Dict[str, Mapping]

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