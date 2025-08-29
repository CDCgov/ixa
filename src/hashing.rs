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
use gxhash::{gxhash128, GxHasher};
use serde::Serialize;
use std::hash::{Hash, Hasher};

pub use gxhash::{HashMap, HashMapExt, HashSet, HashSetExt};

/// A convenience method to compute the hash of a `&str`.
pub fn hash_str(data: &str) -> u64 {
    let mut hasher = GxHasher::default();
    hasher.write(data.as_bytes());
    hasher.finish()
}

// Helper for any T: Hash
pub fn one_shot_128<T: Hash>(value: &T) -> u128 {
    let mut h = GxHasher::default();
    // let mut h = Xxh3Hasher128::default();
    value.hash(&mut h);
    h.finish_u128()
}

pub fn hash_serialized_128<T: Serialize>(value: T) -> u128 {
    let serialized = serialize_to_vec(&value, bincode::config::standard()).unwrap();
    // The `gxhash128` function gives ~3% speedup over `one_shot_128` on my machine on the
    // `births-death` benchmark. HOWEVER, it is not guaranteed to give the same result as
    // `GxHasher` as used in `one_shot_128(&serialized)`.
    gxhash128(serialized.as_slice(), 42)
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
