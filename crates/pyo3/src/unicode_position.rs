#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Offset {
  char_offset: usize,
  byte_offset: usize,
  len: u8,
}

pub struct UnicodePosition {
  offsets: Box<[Offset]>,
}

impl UnicodePosition {
  pub fn new(s: &str) -> Self {
    let mut byte_offset = 0;
    let mut offsets = Vec::new();
    for (char_offset, ch) in s.chars().enumerate() {
      let len = ch.len_utf8();
      if len > 1 {
        offsets.push(Offset {
          char_offset,
          byte_offset,
          len: len as u8,
        });
      }
      byte_offset += len;
    }
    Self {
      offsets: offsets.into_boxed_slice(),
    }
  }

  pub fn char_to_byte(&self, char_offset: usize) -> usize {
    // use binary search to find the offsets
    let ret = self
      .offsets
      .binary_search_by(|o| o.char_offset.cmp(&char_offset));
    match ret {
      Ok(i) => self.offsets[i].byte_offset,
      Err(0) => char_offset,
      Err(i) => {
        let last = &self.offsets[i - 1];
        let end_offset = last.byte_offset + last.len as usize;
        let diff = char_offset - last.char_offset;
        end_offset + diff - 1
      }
    }
  }
  pub fn byte_to_char(&self, byte_offset: usize) -> usize {
    let ret = self
      .offsets
      .binary_search_by(|o| o.byte_offset.cmp(&byte_offset));
    match ret {
      Ok(i) => self.offsets[i].char_offset,
      Err(0) => byte_offset,
      Err(i) => {
        let last = &self.offsets[i - 1];
        let end_offset = last.byte_offset + last.len as usize;
        let diff = byte_offset - end_offset;
        last.char_offset + diff + 1
      }
    }
  }
}
