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

pub use rustc_hash::FxHashMap as HashMap;
pub use rustc_hash::FxHashSet as HashSet;
use std::hash::Hasher;

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
