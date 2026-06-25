//! Index types for property-value lookups.

use crate::entity::{Entity, EntityId};
use crate::hashing::IndexSet;
use crate::prelude::Property;

mod full_index;
mod value_count_index;

pub use full_index::*;
pub use value_count_index::*;

#[derive(Debug)]
pub enum IndexSetResult<'a, E: Entity> {
    /// The index type cannot satisfy the query.
    Unsupported,
    /// The set is empty.
    Empty,
    /// A reference to the index set.
    Set(&'a IndexSet<EntityId<E>>),
}

#[derive(PartialEq, Eq, Debug)]
pub enum IndexCountResult {
    /// The index type cannot satisfy the query.
    Unsupported,
    /// The count of entities.
    Count(usize),
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum PropertyIndexType {
    Unindexed,
    FullIndex,
    ValueCountIndex,
}

pub trait PropertyIndex<E: Entity, P: Property<E>> {
    #[must_use]
    fn index_type(&self) -> PropertyIndexType;

    #[must_use]
    fn get_index_set_result(&self, value: &P) -> IndexSetResult<'_, E>;

    #[must_use]
    fn get_index_count_result(&self, value: &P) -> IndexCountResult;

    fn remove_entity(&mut self, value: &P, entity_id: EntityId<E>);

    fn add_entity(&mut self, value: &P, entity_id: EntityId<E>);

    #[must_use]
    fn max_indexed(&self) -> usize;

    fn set_max_indexed(&mut self, max_indexed: usize);
}
