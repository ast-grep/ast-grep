from __future__ import annotations

from typing import List, TypedDict,  Literal, Dict, Union, Mapping, Optional
from .ast_grep_py import SgNode, SgRoot, Pos, Range, Edit, register_dynamic_language

Strictness = Union[Literal["cst"], Literal["smart"], Literal["ast"], Literal["relaxed"], Literal["signature"]]

class Pattern(TypedDict):
    selector: Optional[str]
    strictness: Optional[Strictness]
    context: str

class NthChild(TypedDict):
    position: int | str
    ofRule: Rule
    nth: int

class PosRule(TypedDict):
    line: int
    column: int

class RangeRule(TypedDict):
    start: PosRule
    end: PosRule

class RuleWithoutNot(TypedDict, total=False):
    # atomic rule
    pattern: str | Pattern
    kind: str
    regex: str
    nthChild: int | str | NthChild
    range: RangeRule

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

class CustomLang(TypedDict, total=False):
  library_path: str
  language_symbol: Optional[str]
  meta_var_char: Optional[str]
  expando_char: Optional[str]

__all__ = [
    "Rule",
    "Config",
    "Relation",
    "Pattern",
    "NthChild",
    "SgNode",
    "SgRoot",
    "Pos",
    "Range",
    "Edit",
    "register_dynamic_language",
]