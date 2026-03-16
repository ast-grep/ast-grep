use bit_set::BitSet;

/// A fast fixed-size bitset for kind filtering.
/// Uses inline `[u64; 8]` storage (512 bits) when all kinds fit,
/// falling back to heap-allocated `BitSet` for larger grammars.
#[derive(Clone, Debug)]
pub enum KindMask {
  /// Inline storage: 512 bits covers all known tree-sitter grammars (200–500 kinds)
  Inline([u64; 8]),
  /// Heap fallback for grammars with kind IDs ≥ 512
  Heap(BitSet),
}

impl KindMask {
  pub fn new() -> Self {
    KindMask::Inline([0u64; 8])
  }

  pub fn from_bitset(set: &BitSet) -> Self {
    let max_bit = set.iter().last().unwrap_or(0);
    if max_bit < 512 {
      let mut words = [0u64; 8];
      for bit in set.iter() {
        let word = bit / 64;
        let bit_in_word = bit % 64;
        words[word] |= 1u64 << bit_in_word;
      }
      KindMask::Inline(words)
    } else {
      KindMask::Heap(set.clone())
    }
  }

  #[inline(always)]
  pub fn contains(&self, bit: usize) -> bool {
    match self {
      KindMask::Inline(words) => {
        if bit >= 512 {
          return false;
        }
        let word = bit / 64;
        let bit_in_word = bit % 64;
        words[word] & (1u64 << bit_in_word) != 0
      }
      KindMask::Heap(set) => set.contains(bit),
    }
  }

  #[inline]
  pub fn insert(&mut self, bit: usize) {
    match self {
      KindMask::Inline(words) => {
        if bit >= 512 {
          let mut set = Self::inline_to_bitset(words);
          set.insert(bit);
          *self = KindMask::Heap(set);
        } else {
          let word = bit / 64;
          let bit_in_word = bit % 64;
          words[word] |= 1u64 << bit_in_word;
        }
      }
      KindMask::Heap(set) => {
        set.insert(bit);
      }
    }
  }

  #[inline]
  pub fn union_with(&mut self, other: &KindMask) {
    match (&mut *self, other) {
      (KindMask::Inline(a), KindMask::Inline(b)) => {
        for i in 0..8 {
          a[i] |= b[i];
        }
      }
      (this @ KindMask::Inline(_), KindMask::Heap(b)) => {
        let mut set = b.clone();
        if let KindMask::Inline(words) = &*this {
          Self::merge_inline_into_bitset(words, &mut set);
        }
        *this = KindMask::Heap(set);
      }
      (KindMask::Heap(a), KindMask::Inline(words)) => {
        Self::merge_inline_into_bitset(words, a);
      }
      (KindMask::Heap(a), KindMask::Heap(b)) => {
        a.union_with(b);
      }
    }
  }

  fn inline_to_bitset(words: &[u64; 8]) -> BitSet {
    let mut set = BitSet::new();
    Self::merge_inline_into_bitset(words, &mut set);
    set
  }

  fn merge_inline_into_bitset(words: &[u64; 8], set: &mut BitSet) {
    for (i, &word) in words.iter().enumerate() {
      let mut w = word;
      let mut bit = i * 64;
      while w != 0 {
        if w & 1 != 0 {
          set.insert(bit);
        }
        w >>= 1;
        bit += 1;
      }
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_inline_basic() {
    let mut mask = KindMask::new();
    assert!(!mask.contains(0));
    assert!(!mask.contains(100));
    mask.insert(0);
    mask.insert(100);
    mask.insert(511);
    assert!(mask.contains(0));
    assert!(mask.contains(100));
    assert!(mask.contains(511));
    assert!(!mask.contains(1));
    assert!(!mask.contains(200));
  }

  #[test]
  fn test_from_bitset() {
    let mut bs = BitSet::new();
    bs.insert(5);
    bs.insert(100);
    bs.insert(300);
    let mask = KindMask::from_bitset(&bs);
    assert!(matches!(mask, KindMask::Inline(_)));
    assert!(mask.contains(5));
    assert!(mask.contains(100));
    assert!(mask.contains(300));
    assert!(!mask.contains(6));
  }

  #[test]
  fn test_promote_to_heap() {
    let mut mask = KindMask::new();
    mask.insert(5);
    mask.insert(600);
    assert!(matches!(mask, KindMask::Heap(_)));
    assert!(mask.contains(5));
    assert!(mask.contains(600));
    assert!(!mask.contains(6));
  }

  #[test]
  fn test_union_inline() {
    let mut a = KindMask::new();
    a.insert(1);
    a.insert(100);
    let mut b = KindMask::new();
    b.insert(2);
    b.insert(200);
    a.union_with(&b);
    assert!(a.contains(1));
    assert!(a.contains(2));
    assert!(a.contains(100));
    assert!(a.contains(200));
  }

  #[test]
  fn test_from_bitset_large() {
    let mut bs = BitSet::new();
    bs.insert(600);
    let mask = KindMask::from_bitset(&bs);
    assert!(matches!(mask, KindMask::Heap(_)));
    assert!(mask.contains(600));
  }

  #[test]
  fn test_inline_contains_out_of_range() {
    let mask = KindMask::new();
    assert!(!mask.contains(512));
    assert!(!mask.contains(1000));
  }
}
