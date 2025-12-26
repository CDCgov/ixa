#![allow(dead_code)]
//! The utilities in this module are used by query and multi-properties so that queries
//! having multiple properties can be resolved to an indexed multi-property if possible.

use std::any::TypeId;
use std::cell::RefCell;
use std::sync::{LazyLock, Mutex};

use crate::hashing::{one_shot_128, HashMap, HashValueType};

/// A map from a list of `TypeId`s to the `index` of the multi-property
/// type. The list of `TypeId`s is assumed to be sorted.
///
/// Use `register_type_ids_to_muli_property_id()` to register a multi-property.
/// We could instead just rely on `TypeId::of::<P::CanonicalValue>()`, but this
/// allows us to determine the type dynamically, e.g. for the web API or debug
/// console.
static MULTI_PROPERTY_INDEX_MAP: LazyLock<Mutex<RefCell<HashMap<HashValueType, usize>>>> =
    LazyLock::new(|| Mutex::new(RefCell::new(HashMap::default())));

/// A method that looks up the `TypeId` of the multi-property that has the given
/// list of `TypeId`s as its properties.
pub fn type_ids_to_multi_property_index(type_ids: &[TypeId]) -> Option<usize> {
    let hash = one_shot_128(&type_ids);
    MULTI_PROPERTY_INDEX_MAP
        .lock()
        .unwrap()
        .borrow()
        .get(&hash)
        .copied()
}

/// A method that registers the `TypeId` of the multi-property tuple type that has the given
/// list of `TypeId`s as its properties.
///
/// Use `type_ids_to_muli_property_id()` to look up a `TypeId`.
pub fn register_type_ids_to_muli_property_index(type_ids: &[TypeId], index: usize) {
    let hash = one_shot_128(&type_ids);
    MULTI_PROPERTY_INDEX_MAP
        .lock()
        .unwrap()
        .borrow_mut()
        .insert(hash, index);
}
