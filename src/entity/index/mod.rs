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

pub enum PropertyIndex<E: Entity, P: Property<E>> {
    Unindexed,
    FullIndex(FullIndex<E, P>),
    ValueCountIndex(ValueCountIndex<E, P>),
}

impl<E: Entity, P: Property<E>> PropertyIndex<E, P> {
    pub fn index_type(&self) -> PropertyIndexType {
        match self {
            Self::Unindexed => PropertyIndexType::Unindexed,
            Self::FullIndex(_) => PropertyIndexType::FullIndex,
            Self::ValueCountIndex(_) => PropertyIndexType::ValueCountIndex,
        }
    }

    pub fn get_index_set_result(&self, value: &P::CanonicalValue) -> IndexSetResult<'_, E> {
        match self {
            Self::Unindexed => IndexSetResult::<'_, E>::Unsupported,
            Self::FullIndex(index) => match index.get(value) {
                Some(set) => IndexSetResult::Set(set),
                None => IndexSetResult::Empty,
            },
            Self::ValueCountIndex(_) => IndexSetResult::<'_, E>::Unsupported,
        }
    }

    pub fn get_index_count_result(&self, value: &P::CanonicalValue) -> IndexCountResult {
        match self {
            Self::Unindexed => IndexCountResult::Unsupported,
            Self::FullIndex(index) => {
                let count = index.get(value).map_or(0, |set| set.len());
                IndexCountResult::Count(count)
            }
            Self::ValueCountIndex(index) => IndexCountResult::Count(index.get(value).unwrap_or(0)),
        }
    }

    pub fn remove_entity(&mut self, value: &P::CanonicalValue, entity_id: EntityId<E>) {
        match self {
            Self::Unindexed => {}
            Self::FullIndex(index) => index.remove_entity(value, entity_id),
            Self::ValueCountIndex(index) => index.remove_entity(value, entity_id),
        }
    }

    pub fn add_entity(&mut self, value: &P::CanonicalValue, entity_id: EntityId<E>) {
        match self {
            Self::Unindexed => {}
            Self::FullIndex(index) => {
                index.add_entity(value, entity_id);
            }
            Self::ValueCountIndex(index) => {
                index.add_entity(value, entity_id);
            }
        }
    }

    /// Returns `None` if there is no index.
    pub fn max_indexed(&self) -> Option<usize> {
        match self {
            Self::Unindexed => None,
            Self::FullIndex(index) => Some(index.max_indexed),
            Self::ValueCountIndex(index) => Some(index.max_indexed),
        }
    }

    pub fn set_max_indexed(&mut self, max_indexed: usize) {
        match self {
            Self::Unindexed => {}
            Self::FullIndex(index) => index.max_indexed = max_indexed,
            Self::ValueCountIndex(index) => index.max_indexed = max_indexed,
        }
    }
}
