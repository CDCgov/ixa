//! Value-count index that maintains only counts per distinct property value.

use std::ops::AddAssign;

use log::{error, trace};

use crate::entity::{Entity, EntityId};
use crate::hashing::HashMap;
use crate::prelude::Property;

#[derive(Default)]
pub struct ValueCountIndex<E: Entity, P: Property<E>> {
    data: HashMap<P::CanonicalValue, usize>,
    pub(in crate::entity) max_indexed: usize,
}

impl<E: Entity, P: Property<E>> ValueCountIndex<E, P> {
    #[must_use]
    pub fn new() -> Self {
        Self {
            data: HashMap::default(),
            max_indexed: 0,
        }
    }

    /// Increments the count for `key`.
    pub fn add_entity(&mut self, key: &P::CanonicalValue, entity_id: EntityId<E>) {
        trace!("adding entity {:?} to index {}", entity_id, P::name());
        self.data.entry(*key).or_default().add_assign(1);
    }

    pub fn remove_entity(&mut self, key: &P::CanonicalValue, entity_id: EntityId<E>) {
        if let Some(count) = self.data.get_mut(key) {
            if *count == 0 {
                error!(
                    "attempted to remove entity {:?} from value-count index with count 0",
                    entity_id
                );
                return;
            }
            *count -= 1;
            if *count == 0 {
                self.data.remove(key);
            }
        }
    }

    pub fn get(&self, key: &P::CanonicalValue) -> Option<usize> {
        self.data.get(key).copied()
    }
}

#[cfg(test)]
mod tests {
    use crate::entity::index::ValueCountIndex;
    use crate::entity::PropertyIndexType;
    use crate::hashing::one_shot_128;
    use crate::prelude::*;
    use crate::with;

    define_entity!(Person);
    define_property!(struct Age(pub u8), Person, default_const = Age(0));
    define_property!(struct Weight(pub u8), Person, default_const = Weight(0));
    define_property!(struct Height(pub u8), Person, default_const = Height(0));

    define_multi_property!((Age, Weight, Height), Person);
    define_multi_property!((Weight, Height, Age), Person);

    type AWH = (Age, Weight, Height);
    type WHA = (Weight, Height, Age);

    #[test]
    fn test_multi_property_index_typed_api() {
        let mut context = Context::new();
        let property_store = context.entity_store.get_property_store_mut::<Person>();

        property_store.set_property_indexed::<AWH>(PropertyIndexType::ValueCountIndex);
        property_store.set_property_indexed::<WHA>(PropertyIndexType::ValueCountIndex);

        context
            .add_entity(with!(Person, Age(1u8), Weight(2u8), Height(3u8)))
            .unwrap();

        assert_eq!(
            context.query_entity_count(with!(Person, Age(1u8), Weight(2u8), Height(3u8))),
            1
        );
        assert_eq!(
            context.query_entity_count(with!(Person, Weight(2u8), Height(3u8), Age(1u8))),
            1
        );

        context
            .add_entity(with!(Person, Weight(1u8), Height(2u8), Age(3u8)))
            .unwrap();

        assert_eq!(
            context.query_entity_count(with!(Person, Weight(1u8), Height(2u8), Age(3u8))),
            1
        );
        assert_eq!(
            context.query_entity_count(with!(Person, Age(3u8), Weight(1u8), Height(2u8))),
            1
        );
    }

    #[test]
    fn test_index_value_compute_same_values() {
        let value = one_shot_128(&"test value");
        let value2 = one_shot_128(&"test value");
        assert_eq!(value, value2);
    }

    #[test]
    fn test_index_value_compute_different_values() {
        let value1 = Age(42);
        let value2 = Age(43);
        assert_ne!(one_shot_128(&value1), one_shot_128(&value2));
    }

    #[test]
    fn test_add_remove_counts() {
        let mut index: ValueCountIndex<Person, Age> = ValueCountIndex::new();
        let value = Age(10);

        assert_eq!(index.get(&value), None);

        index.add_entity(&value, EntityId::new(0));
        assert_eq!(index.get(&value), Some(1));

        index.add_entity(&value, EntityId::new(1));
        assert_eq!(index.get(&value), Some(2));

        index.remove_entity(&value, EntityId::new(0));
        assert_eq!(index.get(&value), Some(1));

        index.remove_entity(&value, EntityId::new(1));
        assert_eq!(index.get(&value), None);
    }
}
