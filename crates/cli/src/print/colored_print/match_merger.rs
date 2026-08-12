use super::MatchDisplay;
use ast_grep_core::tree_sitter::DisplayContext;

/// merging overlapping/adjacent matches
/// adjacent matches: matches that starts or ends on the same line
pub struct MatchMerger<'a> {
  pub last_start_line: usize,
  pub last_end_line: usize,
  pub last_trailing: &'a str,
  pub last_end_offset: usize,
  last_node_end_offset: usize,
  pub context: (u16, u16),
}

impl<'a> MatchMerger<'a> {
  pub fn new(nm: &MatchDisplay<'a>, (before, after): (u16, u16)) -> Self {
    let display = nm.display_context(before as usize, after as usize);
    let last_start_line = display.start_line + 1;
    let last_end_line = nm.display_end_line();
    let last_trailing = display.trailing;
    let last_end_offset = nm.display_range.end;
    let last_node_end_offset = nm.node_match.range().end;
    Self {
      last_start_line,
      last_end_line,
      last_end_offset,
      last_node_end_offset,
      last_trailing,
      context: (before, after),
    }
  }

  // merge non-overlapping matches but start/end on the same line
  pub fn merge_adjacent(&mut self, nm: &MatchDisplay<'a>) -> Option<usize> {
    let display = self.display(nm);
    let start_line = display.start_line;
    if start_line <= self.last_end_line + self.context.1 as usize {
      let last_end_offset = self.last_end_offset;
      self.last_end_offset = nm.display_range.end;
      self.last_node_end_offset = nm.node_match.range().end;
      self.last_trailing = display.trailing;
      Some(last_end_offset)
    } else {
      None
    }
  }

  pub fn conclude_match(&mut self, nm: &MatchDisplay<'a>) {
    let display = self.display(nm);
    self.last_start_line = display.start_line + 1;
    self.last_end_line = nm.display_end_line();
    self.last_trailing = display.trailing;
    self.last_end_offset = nm.display_range.end;
    self.last_node_end_offset = nm.node_match.range().end;
  }

  #[inline]
  pub fn check_overlapping(&self, nm: &MatchDisplay<'a>) -> bool {
    let range = nm.node_match.range();

    // merge overlapping matches.
    // N.B. range.start == last_node_end_offset does not mean overlapping
    if range.start < self.last_node_end_offset {
      // guaranteed by pre-order
      debug_assert!(range.end <= self.last_node_end_offset);
      true
    } else {
      false
    }
  }

  pub fn display(&self, nm: &MatchDisplay<'a>) -> DisplayContext<'a> {
    let (before, after) = self.context;
    nm.display_context(before as usize, after as usize)
  }
}
