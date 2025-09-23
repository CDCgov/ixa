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

use bincode::serde::encode_to_vec as serialize_to_vec;
pub use rustc_hash::FxHashMap as HashMap;
pub use rustc_hash::FxHashSet as HashSet;
use serde::Serialize;
use std::hash::{Hash, Hasher};
use xxhash_rust::xxh3::Xxh3Default;

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

/// A convenience method to compute the hash of a `&str`.
pub fn hash_str(data: &str) -> u64 {
    let mut hasher = rustc_hash::FxHasher::default();
    hasher.write(data.as_bytes());
    hasher.finish()
}

// Helper for any T: Hash
pub fn one_shot_128<T: Hash>(value: &T) -> u128 {
    let mut h = Xxh3Default::default();
    value.hash(&mut h);
    h.digest128()
}

pub fn hash_serialized_128<T: Serialize>(value: T) -> u128 {
    let serialized = serialize_to_vec(&value, bincode::config::standard()).unwrap();
    // The `xxh3_128` should be a little faster, but it is not guaranteed to produce the same hash.
    // xxh3_128(serialized.as_slice())
    one_shot_128(&serialized.as_slice())
}

#[cfg(test)]
mod tests {
    use super::*;
    use bincode::serde::encode_to_vec as serialize_to_vec;
    use serde::Serialize;

    #[test]
    fn hash_serialized_equals_one_shot() {
        let value = "hello";
        let a = hash_serialized_128(value);
        let serialized = serialize_to_vec(&value, bincode::config::standard()).unwrap();
        let b = one_shot_128(&serialized.as_slice());

        assert_eq!(a, b);
    }

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
    fn serialization_is_concatenation() {
        // We rely on the fact that the serialization of a tuple is the concatenation of the
        // component types, and likewise for structs. This tests that invariant.

        #[derive(Debug, Serialize)]
        struct MyStruct {
            name: &'static str,
            age: i32,
            height: f64,
        }

        let my_struct = MyStruct {
            name: "John",
            age: 25,
            height: 1.80,
        };
        let my_tuple = ("John", 25, 1.80);

        let encoded_struct = serialize_to_vec(my_struct, bincode::config::standard()).unwrap();
        let encoded_tuple = serialize_to_vec(my_tuple, bincode::config::standard()).unwrap();

        assert_eq!(encoded_struct, encoded_tuple);

        let encoded_str = bincode::encode_to_vec("John", bincode::config::standard()).unwrap();
        let encoded_int = bincode::encode_to_vec(25, bincode::config::standard()).unwrap();
        let encoded_float = bincode::encode_to_vec(1.80, bincode::config::standard()).unwrap();
        let flattened = encoded_str
            .iter()
            .copied()
            .chain(encoded_int.iter().copied())
            .chain(encoded_float.iter().copied())
            .collect::<Vec<u8>>();

        assert_eq!(flattened, encoded_tuple);
    }
}
