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

/// Provides API parity with `std::collections::HashMap`.
pub trait HashMapExt {
    #[must_use]
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

    #[must_use]
    fn new() -> Self;

    /// Equivalent to `self.iter().cloned().collect::<Vec<_>>()`.
    #[must_use]
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
#[must_use]
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
#[must_use]
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
