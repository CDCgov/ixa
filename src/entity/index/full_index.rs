//! Full property-value index that maps each distinct value to the set of matching entity IDs.

use log::trace;

use crate::entity::index::{IndexCountResult, IndexSetResult, PropertyIndex};
use crate::entity::{Entity, EntityId, PropertyIndexType};
use crate::hashing::{HashMap, IndexSet};
use crate::prelude::{IndexableProperty, Property};

/// An index that maintains a full set of entity IDs for each distinct property value.
/// The entity IDs are stored in an `IndexSet` for both fast containment checks and fast
/// direct indexing (fast random sampling).
#[derive(Default)]
pub struct FullIndex<E: Entity, P: Property<E>> {
    data: HashMap<P, IndexSet<EntityId<E>>>,

    // The largest person ID that has been indexed. Used so that we
    // can lazily index when a person is added.
    pub(in crate::entity) max_indexed: usize,
}

impl<E: Entity, P: IndexableProperty<E>> FullIndex<E, P> {
    #[must_use]
    pub fn new() -> Self {
        Self {
            data: HashMap::default(),
            max_indexed: 0,
        }
    }

    /// Inserts an entity into the set associated with `key`, creating a new set if one does not yet
    /// exist.
    pub fn add_entity(&mut self, key: &P, entity_id: EntityId<E>) {
        trace!("adding entity {:?} to index {}", entity_id, P::name());
        self.data.entry(*key).or_default().insert(entity_id);
    }

    pub fn remove_entity(&mut self, key: &P, entity_id: EntityId<E>) {
        if let Some(set) = self.data.get_mut(key) {
            set.swap_remove(&entity_id);
            // Clean up the entry if there are no entities
            if set.is_empty() {
                self.data.remove(key);
            }
        }
    }

    #[must_use]
    pub fn get(&self, key: &P) -> Option<&IndexSet<EntityId<E>>> {
        self.data.get(key)
    }
}

impl<E, P> PropertyIndex<E, P> for FullIndex<E, P>
where
    E: Entity,
    P: IndexableProperty<E>,
{
    fn index_type(&self) -> PropertyIndexType {
        PropertyIndexType::FullIndex
    }

    fn get_index_set_result(&self, value: &P) -> IndexSetResult<'_, E> {
        match self.get(value) {
            Some(set) => IndexSetResult::Set(set),
            None => IndexSetResult::Empty,
        }
    }

    fn get_index_count_result(&self, value: &P) -> IndexCountResult {
        let count = self.get(value).map_or(0, |set| set.len());
        IndexCountResult::Count(count)
    }

    fn remove_entity(&mut self, value: &P, entity_id: EntityId<E>) {
        FullIndex::remove_entity(self, value, entity_id);
    }

    fn add_entity(&mut self, value: &P, entity_id: EntityId<E>) {
        FullIndex::add_entity(self, value, entity_id);
    }

    fn max_indexed(&self) -> usize {
        self.max_indexed
    }

    fn set_max_indexed(&mut self, max_indexed: usize) {
        self.max_indexed = max_indexed;
    }
}

#[cfg(test)]
mod tests {
    // Tests in `src/entity/query.rs` also exercise indexing code.
    use crate::prelude::*;
    use crate::with;

    define_entity!(Person);
    define_property!(struct Age(u8), Person, default_const = Age(0));
    define_property!(struct Weight(u8), Person, default_const = Weight(0));
    define_property!(struct Height(u8), Person, default_const = Height(0));

    define_multi_property!((Age, Weight, Height), Person);
    define_multi_property!((Weight, Height, Age), Person);

    type AWH = (Age, Weight, Height);
    type WHA = (Weight, Height, Age);

    #[test]
    fn test_multi_property_index_typed_api() {
        let mut context = Context::new();

        assert_ne!(AWH::type_id(), WHA::type_id());
        context.index_property::<Person, AWH>();

        context
            .add_entity(with!(Person, Age(1u8), Weight(2u8), Height(3u8)))
            .unwrap();

        let mut results_a = Default::default();
        context.with_query_results(
            with!(Person, Age(1u8), Weight(2u8), Height(3u8)),
            &mut |results| results_a = results.into_iter().collect::<Vec<_>>(),
        );
        assert_eq!(results_a.len(), 1);

        let mut results_b = Default::default();
        context.with_query_results(
            with!(Person, Weight(2u8), Height(3u8), Age(1u8)),
            &mut |results| results_b = results.into_iter().collect::<Vec<_>>(),
        );
        assert_eq!(results_b.len(), 1);

        assert_eq!(results_a, results_b);

        context
            .add_entity(with!(Person, Weight(1u8), Height(2u8), Age(3u8)))
            .unwrap();

        let mut results_a = Default::default();
        context.with_query_results(
            with!(Person, Weight(1u8), Height(2u8), Age(3u8)),
            &mut |results| results_a = results.into_iter().collect::<Vec<_>>(),
        );
        assert_eq!(results_a.len(), 1);

        let mut results_b = Default::default();
        context.with_query_results(
            with!(Person, Age(3u8), Weight(1u8), Height(2u8)),
            &mut |results| results_b = results.into_iter().collect::<Vec<_>>(),
        );
        assert_eq!(results_b.len(), 1);

        assert_eq!(results_a, results_b);
    }

    #[test]
    #[should_panic(expected = "Cannot index multi-property")]
    fn test_indexing_duplicate_multi_property_panics() {
        let mut context = Context::new();
        context.index_property::<Person, WHA>();
    }
}
