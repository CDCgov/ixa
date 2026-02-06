//! Value-count index that maintains only counts per distinct property value.

use crate::entity::{Entity, EntityId, HashValueType};
use crate::prelude::Property;

#[derive(Default)]
pub struct ValueCountIndex<E: Entity, P: Property<E>> {
    pub(in crate::entity) max_indexed: usize,
    #[allow(dead_code)]
    _phantom: std::marker::PhantomData<(E, P)>,
}

impl<E: Entity, P: Property<E>> ValueCountIndex<E, P> {
    #[must_use]
    pub fn new() -> Self {
        Self {
            max_indexed: 0,
            _phantom: std::marker::PhantomData,
        }
    }

    pub fn add_entity(&mut self, _key: &P::CanonicalValue, _entity_id: EntityId<E>) -> bool {
        false
    }

    pub fn remove_entity(&mut self, _key: &P::CanonicalValue, _entity_id: EntityId<E>) {}

    pub fn add_entity_with_hash(&mut self, _hash: HashValueType, _entity_id: EntityId<E>) {}

    pub fn remove_entity_with_hash(&mut self, _hash: HashValueType, _entity_id: EntityId<E>) {}

    pub fn get_with_hash(&self, _hash: HashValueType) -> Option<usize> {
        None
    }
}
