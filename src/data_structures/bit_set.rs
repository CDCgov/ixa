//! A semi-dynamic bit mask data structure the size of which is fixed at construction.
//!
//! Supports very efficient "is empty" check, `get`, and `set` operations. A `BitSet` allocates all
//! of its storage up front. The storage is chunked into 64 bit words, which means allocated
//! capacity can be greater than the value of `bit_count` the `BitSet` was constructed with. For
//! simplicity, we allow the entire capacity to be used (with `set` and `get`). As a consequence,
//! the actual value of `bit_count` the `BitSet` was constructed with is not recoverable after
//! construction.
//!
//! ## Implementation
//!
//! Storage is inline for <=64 bits, which makes access and cloning very cheap for the
//! overwhelmingly common case. Internally we use a `set_count` to make `is_empty` very fast
//! independent of whether or not storage is inline.

pub struct BitSet {
    /// 64 bits inline or heap allocated spillover.
    storage: Storage,
    /// Count of set bits, for O(1) empty check. Maintained in mutation operations.
    set_count: u32,
}

enum Storage {
    /// Inline storage for bit counts <= 64. No heap allocation; `Clone` is a register copy.
    Inline(u64),
    /// Heap storage for bit counts > 64. Fixed length, allocated once.
    Heap(Box<[u64]>),
}

impl BitSet {
    pub fn new(bit_count: usize) -> Self {
        let storage = if bit_count <= 64 {
            Storage::Inline(0)
        } else {
            let num_words = bit_count.div_ceil(64);
            Storage::Heap(vec![0u64; num_words].into_boxed_slice())
        };
        Self {
            storage,
            set_count: 0,
        }
    }

    #[inline(always)]
    pub fn is_empty(&self) -> bool {
        self.set_count == 0
    }

    /// Clones this `BitSet` if it contains any set bits.
    #[inline(always)]
    pub fn clone_if_nonempty(&self) -> Option<Self> {
        if self.is_empty() {
            None
        } else {
            Some(self.clone())
        }
    }

    /// Returns the total number of bits that can be stored in this `BitSet`.
    #[inline(always)]
    pub fn capacity(&self) -> usize {
        match &self.storage {
            Storage::Inline(_) => 64,
            Storage::Heap(words) => words.len() * 64,
        }
    }

    /// Returns `true` if the `n`th bit is set, `false` otherwise.
    #[inline(always)]
    pub fn get(&self, n: usize) -> bool {
        match &self.storage {
            Storage::Inline(word) => (word >> n) & 1 != 0,
            Storage::Heap(words) => {
                let word = n >> 6; // n / 64
                let bit = n & 63; // n % 64
                (words[word] >> bit) & 1 != 0
            }
        }
    }

    /// Sets the `n`th bit.
    #[inline(always)]
    pub fn set(&mut self, n: usize) {
        match &mut self.storage {
            Storage::Inline(word) => {
                let mask = 1u64 << n;
                if *word & mask == 0 {
                    *word |= mask;
                    self.set_count += 1;
                }
            }
            Storage::Heap(words) => {
                let idx = n >> 6;
                let mask = 1u64 << (n & 63);
                let prev = words[idx];
                if prev & mask == 0 {
                    words[idx] = prev | mask;
                    self.set_count += 1;
                }
            }
        }
    }

    /// Clears the `n`th bit.
    #[inline(always)]
    pub fn reset(&mut self, n: usize) {
        match &mut self.storage {
            Storage::Inline(word) => {
                let mask = 1u64 << n;
                if *word & mask != 0 {
                    *word &= !mask;
                    self.set_count -= 1;
                }
            }
            Storage::Heap(words) => {
                let idx = n >> 6;
                let mask = 1u64 << (n & 63);
                let prev = words[idx];
                if prev & mask != 0 {
                    words[idx] = prev & !mask;
                    self.set_count -= 1;
                }
            }
        }
    }

    /// Clears the entire `BitSet`, resetting every bit.
    pub fn clear(&mut self) {
        match &mut self.storage {
            Storage::Inline(word) => *word = 0,
            Storage::Heap(words) => {
                for word in words.iter_mut() {
                    *word = 0;
                }
            }
        }
        self.set_count = 0;
    }
}

// We manually implement `Clone` in order to decorate with `#[inline]`. Deriving `Clone` would
// generate essentially the same code but with weaker confidence of inlining. (Code complexity,
// compiler version, platform, whether LTO is enabled, and other mysterious factors contribute
// to the inlining decision threshold in the general case.)
impl Clone for BitSet {
    #[inline]
    fn clone(&self) -> Self {
        let storage = match &self.storage {
            // Register copy, no allocation.
            Storage::Inline(word) => Storage::Inline(*word),
            // Allocates. Only reached for bit counts > 64.
            Storage::Heap(words) => Storage::Heap(words.clone()),
        };
        Self {
            storage,
            set_count: self.set_count,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::BitSet;

    #[test]
    fn capacity_is_at_least_one_word_and_rounds_up_to_whole_words() {
        for (bit_count, expected_capacity) in [
            (0, 64),
            (1, 64),
            (64, 64),
            (65, 128),
            (128, 128),
            (129, 192),
        ] {
            let bit_set = BitSet::new(bit_count);
            assert_eq!(bit_set.capacity(), expected_capacity);
            assert!(bit_set.is_empty());
        }
    }

    #[test]
    fn set_and_get_inline_bits() {
        let mut bit_set = BitSet::new(64);

        for bit in [0, 1, 31, 32, 63] {
            assert!(!bit_set.get(bit));
            bit_set.set(bit);
            assert!(bit_set.get(bit));
        }

        assert!(!bit_set.is_empty());
        assert!(!bit_set.get(2));
        assert!(!bit_set.get(62));
    }

    #[test]
    fn set_and_get_heap_bits_across_word_boundaries() {
        let mut bit_set = BitSet::new(130);

        for bit in [0, 63, 64, 65, 127, 128, 129, 191] {
            assert!(!bit_set.get(bit));
            bit_set.set(bit);
            assert!(bit_set.get(bit));
        }

        assert!(!bit_set.is_empty());
        assert!(!bit_set.get(1));
        assert!(!bit_set.get(66));
        assert!(!bit_set.get(190));
    }

    #[test]
    fn repeated_set_and_reset_are_idempotent() {
        let mut bit_set = BitSet::new(65);

        bit_set.set(64);
        bit_set.set(64);
        bit_set.set(127);
        bit_set.reset(64);
        bit_set.reset(64);

        assert!(!bit_set.get(64));
        assert!(bit_set.get(127));
        assert!(!bit_set.is_empty());

        bit_set.reset(127);
        assert!(bit_set.is_empty());

        bit_set.reset(127);
        assert!(bit_set.is_empty());
    }

    #[test]
    fn clear_resets_inline_storage_and_allows_reuse() {
        let mut bit_set = BitSet::new(64);
        for bit in [0, 32, 63] {
            bit_set.set(bit);
        }

        bit_set.clear();

        assert!(bit_set.is_empty());
        for bit in [0, 32, 63] {
            assert!(!bit_set.get(bit));
        }

        bit_set.set(32);
        assert!(bit_set.get(32));
        assert!(!bit_set.is_empty());
    }

    #[test]
    fn clear_resets_every_heap_word_and_allows_reuse() {
        let mut bit_set = BitSet::new(192);
        for bit in [0, 63, 64, 127, 128, 191] {
            bit_set.set(bit);
        }

        bit_set.clear();

        assert!(bit_set.is_empty());
        for bit in [0, 63, 64, 127, 128, 191] {
            assert!(!bit_set.get(bit));
        }

        bit_set.set(128);
        assert!(bit_set.get(128));
        assert!(!bit_set.is_empty());
    }

    #[test]
    fn inline_clone_is_independent() {
        assert_clone_is_independent(64, 1, 63);
    }

    #[test]
    fn heap_clone_is_independent() {
        assert_clone_is_independent(128, 64, 127);
    }

    #[test]
    fn clone_if_nonempty_returns_none_for_empty_inline_and_heap_storage() {
        assert!(BitSet::new(64).clone_if_nonempty().is_none());
        assert!(BitSet::new(65).clone_if_nonempty().is_none());
    }

    #[test]
    fn clone_if_nonempty_returns_an_independent_clone() {
        let mut original = BitSet::new(65);
        original.set(64);

        let mut cloned = original.clone_if_nonempty().unwrap();
        cloned.reset(64);
        cloned.set(65);

        assert!(original.get(64));
        assert!(!original.get(65));
        assert!(!cloned.get(64));
        assert!(cloned.get(65));
    }

    fn assert_clone_is_independent(bit_count: usize, original_bit: usize, clone_bit: usize) {
        let mut original = BitSet::new(bit_count);
        original.set(original_bit);

        let mut cloned = original.clone();
        cloned.reset(original_bit);
        cloned.set(clone_bit);

        assert!(original.get(original_bit));
        assert!(!original.get(clone_bit));
        assert!(!cloned.get(original_bit));
        assert!(cloned.get(clone_bit));
        assert!(!original.is_empty());
        assert!(!cloned.is_empty());
    }
}
