//! This module provides a deterministic hasher and `HashMap` and `HashSet` variants that use
//! it. The hashing data structures in the standard library are not deterministic:
//!
//! > By default, HashMap uses a hashing algorithm selected to provide
//! > resistance against HashDoS attacks. The algorithm is randomly seeded, and a
//! > reasonable best-effort is made to generate this seed from a high quality,
//! > secure source of randomness provided by the host without blocking the program.
//!
//! The standard library `HashMap` has a `new` method, but `HashMap<K, V, S>` does not have a `new`
//! method by default. Use `HashMap::default()` instead to create a new hashmap with the default
//! hasher. If you really need to keep the API the same across implementations, we provide the
//! `HashMapExt` trait extension. Similarly, for `HashSet` and `HashSetExt`.The traits need only be
//! in scope.
//!

use std::hash::{BuildHasherDefault, Hash, Hasher};

pub use indexmap::set::Iter as IndexSetIter;
use indexmap::IndexSet as RawIndexSet;
pub use rustc_hash::{FxHashMap as HashMap, FxHashSet as HashSet, FxHasher};
use xxhash_rust::xxh3::Xxh3Default;

type FxBuildHasher = BuildHasherDefault<FxHasher>;

pub type IndexSet<T> = RawIndexSet<T, FxBuildHasher>;

pub type HashValueType = u128;

pub(crate) type DeterministicHasher = Xxh3Default;

/// A `rkyv` writer that streams serialized bytes directly into a `Hasher`.
pub struct HasherWriter<'a, H> {
    hasher: &'a mut H,
    pos: usize,
}

impl<'a, H> HasherWriter<'a, H> {
    #[must_use]
    pub fn new(hasher: &'a mut H) -> Self {
        Self { hasher, pos: 0 }
    }
}

impl<H: Hasher> rkyv::ser::Positional for HasherWriter<'_, H> {
    fn pos(&self) -> usize {
        self.pos
    }
}

impl<H: Hasher> rkyv::ser::Writer<rkyv::rancor::Error> for HasherWriter<'_, H> {
    fn write(&mut self, bytes: &[u8]) -> Result<(), rkyv::rancor::Error> {
        self.hasher.write(bytes);
        self.pos += bytes.len();
        Ok(())
    }
}

/// A fixed-size `rkyv` writer used by macro-generated equality implementations.
#[derive(Debug, Clone, Copy)]
pub struct EqualityBufferWriter<const N: usize> {
    buf: [u8; N],
    pos: usize,
}

impl<const N: usize> EqualityBufferWriter<N> {
    #[must_use]
    pub fn new() -> Self {
        Self {
            buf: [0; N],
            pos: 0,
        }
    }

    #[must_use]
    pub fn as_written(&self) -> &[u8] {
        &self.buf[..self.pos]
    }
}

impl<const N: usize> Default for EqualityBufferWriter<N> {
    fn default() -> Self {
        Self::new()
    }
}

impl<const N: usize> rkyv::ser::Positional for EqualityBufferWriter<N> {
    fn pos(&self) -> usize {
        self.pos
    }
}

impl<const N: usize, E: rkyv::rancor::Source> rkyv::ser::Writer<E> for EqualityBufferWriter<N> {
    fn write(&mut self, bytes: &[u8]) -> Result<(), E> {
        let end = self.pos + bytes.len();
        assert!(
            end <= N,
            "serialized form exceeded fixed buffer size: {} > {}",
            end,
            N
        );
        self.buf[self.pos..end].copy_from_slice(bytes);
        self.pos = end;
        Ok(())
    }
}

/// Provides API parity with `std::collections::HashMap`.
pub trait HashMapExt {
    fn new() -> Self;
}

impl<K, V> HashMapExt for HashMap<K, V> {
    fn new() -> Self {
        HashMap::default()
    }
}

// Note that trait aliases are not yet stabilized in rustc.
// See https://github.com/rust-lang/rust/issues/41517
/// Provides API parity with `std::collections::HashSet`.
pub trait HashSetExt {
    type Item;

    fn new() -> Self;

    /// Equivalent to `self.iter().cloned().collect::<Vec<_>>()`.
    fn to_owned_vec(&self) -> Vec<Self::Item>;
}

impl<T: Clone> HashSetExt for HashSet<T> {
    type Item = T;

    fn new() -> Self {
        HashSet::default()
    }

    fn to_owned_vec(&self) -> Vec<Self::Item> {
        self.iter().cloned().collect()
    }
}

impl<T: Clone> HashSetExt for IndexSet<T> {
    type Item = T;

    fn new() -> Self {
        IndexSet::default()
    }

    fn to_owned_vec(&self) -> Vec<Self::Item> {
        self.iter().cloned().collect()
    }
}

/// A convenience method to compute the hash of a `&str`.
pub fn hash_str(data: &str) -> u64 {
    let mut hasher = rustc_hash::FxHasher::default();
    hasher.write(data.as_bytes());
    hasher.finish()
}

/// This if factored out for cases of implementations using the `Hash` trait to compute a 128-bit
/// hash. This provides a unified interface to the implementation-specific 128-bit "finish" method.
#[must_use]
pub(crate) fn finish_deterministic_hash_128(hasher: DeterministicHasher) -> HashValueType {
    hasher.digest128()
}

/// Helper for any `T: Hash` using the crate's deterministic hasher.
pub fn one_shot_128<T: Hash>(value: &T) -> u128 {
    let mut hasher = DeterministicHasher::default();
    value.hash(&mut hasher);
    finish_deterministic_hash_128(hasher)
}

#[cfg(test)]
mod tests {
    use std::hash::Hash;

    use super::*;

    #[test]
    fn hashes_strings() {
        let a = one_shot_128(&"hello");
        let b = one_shot_128(&"hello");
        let c = one_shot_128(&"world");
        assert_eq!(a, b);
        assert_ne!(a, c);
    }

    #[test]
    fn hashes_structs() {
        #[derive(Hash)]
        struct S {
            x: u32,
            y: String,
        }
        let h1 = one_shot_128(&S {
            x: 1,
            y: "a".into(),
        });
        let h2 = one_shot_128(&S {
            x: 1,
            y: "a".into(),
        });
        assert_eq!(h1, h2);
    }

    #[test]
    fn hashing_tuple_matches_hashing_components_in_order() {
        let tuple = ("John", 25_u32, true);

        let mut hasher = DeterministicHasher::default();
        tuple.0.hash(&mut hasher);
        tuple.1.hash(&mut hasher);
        tuple.2.hash(&mut hasher);

        assert_eq!(finish_deterministic_hash_128(hasher), one_shot_128(&tuple));
    }
}
