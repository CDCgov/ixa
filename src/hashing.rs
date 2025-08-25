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
//! The `hash_usize` free function is a convenience function used in `crate::random::get_rng`.

use bincode::serde::encode_to_vec as serialize_to_vec;
pub use rustc_hash::FxHashMap as HashMap;
pub use rustc_hash::FxHashSet as HashSet;
use serde::Serialize;
use std::hash::{Hash, Hasher};
use twox_hash::XxHash3_128;

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
    fn new() -> Self;
}

impl<T> HashSetExt for HashSet<T> {
    fn new() -> Self {
        HashSet::default()
    }
}

/// A convenience method to compute the hash of a `&str`.
pub fn hash_str(data: &str) -> u64 {
    let mut hasher = rustc_hash::FxHasher::default();
    hasher.write(data.as_bytes());
    hasher.finish()
}

pub struct Xxh3Hasher128(XxHash3_128);

impl Default for Xxh3Hasher128 {
    fn default() -> Self {
        Self(XxHash3_128::new())
        // or Xxh3::with_seed(seed) for domain separation
    }
}

impl Hasher for Xxh3Hasher128 {
    fn write(&mut self, bytes: &[u8]) {
        self.0.write(bytes); // stream bytes, no allocation
    }
    // Hasher requires a u64 result; return the low 64 bits of the 128-bit digest.
    #[allow(clippy::cast_possible_truncation)]
    fn finish(&self) -> u64 {
        self.0.finish_128() as u64
    }
}

impl Xxh3Hasher128 {
    pub fn finish_u128(self) -> u128 {
        // consume the state to produce the 128-bit digest
        self.0.finish_128()
    }
}

// Helper for any T: Hash
pub fn one_shot_128<T: Hash>(value: &T) -> u128 {
    let mut h = Xxh3Hasher128::default();
    value.hash(&mut h);
    h.finish_u128()
}

pub fn hash_serialized_128<T: Serialize>(value: T) -> u128 {
    let serialized = serialize_to_vec(&value, bincode::config::standard()).unwrap();
    one_shot_128(&serialized)
}

#[cfg(test)]
mod tests {
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
}
