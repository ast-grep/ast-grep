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
  def find(self, config = None, **kwargs) -> SgNode: ...