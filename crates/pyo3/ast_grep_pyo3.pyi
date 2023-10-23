from typing import Optional, TypedDict, Unpack, NotRequired

class Pattern(TypedDict):
  selector: str
  context: str

class Rule(TypedDict):
  pattern: NotRequired[str | Pattern]
  kind: NotRequired[str]

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

class SgNode:
  def range(self) -> Range: pass
  def find(self, config = None, **kwargs: Unpack[Rule]) -> SgNode: ...
  def get_match(self, meta_var: str) -> Optional[SgNode]: ...
  def text(self) -> str: ...