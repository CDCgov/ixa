use std::any::TypeId;

use seq_macro::seq;

use crate::entity::multi_property::static_reorder_by_keys;
use crate::entity::property::Property;
use crate::entity::query::property_entity_values::{PropertyEntityValues0, PropertyEntityValues1};
use crate::entity::query::query_result_iterator::QueryResultIterator;
use crate::entity::query::source_set::SourceSet;
use crate::entity::{ContextEntitiesExt, Entity, EntityId, HashValueType, Query};
use crate::hashing::one_shot_128;
use crate::Context;

impl<E: Entity> Query<E> for PropertyEntityValues0<E> {
    fn get_query(&self) -> Vec<(usize, HashValueType)> {
        Vec::new()
    }

    fn get_type_ids(&self) -> Vec<TypeId> {
        Vec::new()
    }

    fn multi_property_id(&self) -> Option<usize> {
        None
    }

    fn multi_property_value_hash(&self) -> HashValueType {
        let empty: &[u128] = &[];
        one_shot_128(&empty)
    }

    fn new_query_result_iterator<'c>(&self, context: &'c Context) -> QueryResultIterator<'c, E> {
        let population_iterator = context.get_entity_iterator::<E>();
        QueryResultIterator::from_population_iterator(population_iterator)
    }

    fn match_entity(&self, _entity_id: EntityId<E>, _context: &Context) -> bool {
        // Every entity matches the empty query.
        true
    }

    fn filter_entities(&self, _entities: &mut Vec<EntityId<E>>, _context: &Context) {
        // Nothing to do.
    }
}

// Implement the query version with one parameter.
impl<E: Entity, P0: Property<E>> Query<E> for PropertyEntityValues1<E, P0> {
    fn get_query(&self) -> Vec<(usize, HashValueType)> {
        let value = P0::make_canonical(self._0);
        vec![(P0::id(), P0::hash_property_value(&value))]
    }

    fn get_type_ids(&self) -> Vec<TypeId> {
        vec![P0::type_id()]
    }

    fn multi_property_id(&self) -> Option<usize> {
        // While not a "true" multi-property, it is convenient to have this method return the
        // `TypeId` of the singleton property.
        Some(P0::index_id())
    }

    fn multi_property_value_hash(&self) -> HashValueType {
        P0::hash_property_value(&P0::make_canonical(self._0))
    }

    fn new_query_result_iterator<'c>(&self, context: &'c Context) -> QueryResultIterator<'c, E> {
        let property_store = context.entity_store.get_property_store::<E>();

        // The case of an indexed multi-property.
        // This mirrors the indexed case in `SourceSet<'a, E>::new()`. The difference is, if the
        // multi-property is unindexed, we fall through to create `SourceSet`s for the components
        // rather than wrapping a `DerivedPropertySource`.
        if let Some(multi_property_id) = self.multi_property_id() {
            // The `index_unindexed_people` method returns `false` if the property is not indexed.
            if property_store.index_unindexed_entities_for_property_id(context, multi_property_id) {
                // Fetch the right hash bucket from the index and return it.
                let property_value_store = property_store.get_with_id(multi_property_id);
                if let Some(people_set) =
                    property_value_store.get_index_set_with_hash(self.multi_property_value_hash())
                {
                    return QueryResultIterator::from_index_set(people_set);
                } else {
                    // Since we already checked that this multi-property is indexed, it must be that
                    // there are no entities having this property value.
                    return QueryResultIterator::empty();
                }
            }
            // If the property is not indexed, we fall through.
        }

        // We create a source set for each property.
        let mut sources: Vec<SourceSet<E>> = Vec::new();

        if let Some(source_set) = SourceSet::new::<P0>(self._0, context) {
            sources.push(source_set);
        } else {
            // If a single source set is empty, the intersection of all sources is empty.
            return QueryResultIterator::empty();
        }

        QueryResultIterator::from_sources(sources)
    }

    fn match_entity(&self, entity_id: EntityId<E>, context: &Context) -> bool {
        let found_value: P0 = context.get_property(entity_id);
        found_value == self._0
    }

    fn filter_entities(&self, entities: &mut Vec<EntityId<E>>, context: &Context) {
        let property_value_store = context.get_property_value_store::<E, P0>();
        entities.retain(|entity| self._0 == property_value_store.get(*entity));
    }
}

macro_rules! impl_query {
    ($struct_name:ident, $ct:expr) => {
        seq!(N in 0..$ct {
            impl<
                E: Entity,
                #(
                    T~N : Property<E>,
                )*
            > Query<E> for $struct_name<
                E,
                #(
                    T~N,
                )*
            >
            {
                fn get_query(&self) -> Vec<(usize, HashValueType)> {
                    let mut ordered_items = vec![
                    #(
                        (T~N::id(), T~N::hash_property_value(&T~N::make_canonical(self._~N))),
                    )*
                    ];
                    ordered_items.sort_unstable_by(|a, b| a.0.cmp(&b.0));
                    ordered_items
                }

                fn get_type_ids(&self) -> Vec<TypeId> {
                    vec![
                        #(
                            T~N::type_id(),
                        )*
                    ]
                }

                fn multi_property_value_hash(&self) -> HashValueType {
                    // This needs to be kept in sync with how multi-properties compute their hash. We are
                    // exploiting the fact that `bincode` encodes tuples as the concatenation of their
                    // elements. Unfortunately, `bincode` allocates, but we avoid more allocations by
                    // using stack allocated arrays.

                    // Multi-properties order their values by lexicographic order of the component
                    // properties, not `TypeId` order.
                    // let type_ids: [TypeId; $ct] = [
                    //     #(
                    //         T~N::type_id(),
                    //     )*
                    // ];
                    let keys: [&str; $ct] = [
                        #(
                            T~N::name(),
                        )*
                    ];
                    // It is convenient to have the elements of the array to be `Copy` in the `static_apply_reordering`
                    // function. Since references are trivially copyable, we construct `values` below to be an array
                    // of _references_ to the `Vec`s returned from `encode_to_vec`. (The compiler is smart enough to
                    // keep the referenced value in scope.)
                    let mut values: [&Vec<u8>; $ct] = [
                        #(
                            &$crate::bincode::serde::encode_to_vec(self._~N, bincode::config::standard()).unwrap(),
                        )*
                    ];
                    static_reorder_by_keys(&keys, &mut values);

                    let data = values.into_iter().flatten().copied().collect::<Vec<u8>>();
                    one_shot_128(&data.as_slice())
                }

                fn new_query_result_iterator<'c>(&self, context: &'c Context) -> QueryResultIterator<'c, E> {
                    // The case of an indexed multi-property.
                    // This mirrors the indexed case in `SourceSet<'a, E>::new()`. The difference is, if the
                    // multi-property is unindexed, we fall through to create `SourceSet`s for the components
                    // rather than wrapping a `DerivedPropertySource`.
                    if let Some(multi_property_id) = self.multi_property_id() {
                        let property_store = context.entity_store.get_property_store::<E>();
                        // The `index_unindexed_people` method returns `false` if the property is not indexed.
                        if property_store.index_unindexed_entities_for_property_id(context, multi_property_id) {
                            // Fetch the right hash bucket from the index and return it.
                            let property_value_store = property_store.get_with_id(multi_property_id);
                            if let Some(entity_set) = property_value_store.get_index_set_with_hash(
                                self.multi_property_value_hash(),
                            ) {
                                return QueryResultIterator::from_index_set(entity_set);
                            } else {
                                // Since we already checked that this multi-property is indexed, it must be that
                                // there are no entities having this property value.
                                return QueryResultIterator::empty();
                            }
                        }
                        // If the property is not indexed, we fall through.
                    }

                    // We create a source set for each property.
                    let mut sources: Vec<SourceSet<E>> = Vec::new();

                    #(
                        if let Some(source_set) = SourceSet::new::<T~N>(self._~N, context) {
                            sources.push(source_set);
                        } else {
                            // If a single source set is empty, the intersection of all sources is empty.
                            return QueryResultIterator::empty();
                        }
                    )*

                    QueryResultIterator::from_sources(sources)
                }

                fn match_entity(&self, entity_id: EntityId<E>, context: &Context) -> bool {
                    #(
                        {
                            let found_value: T~N = context.get_property(entity_id);
                            if found_value != self._~N {
                                return false
                            }
                        }
                    )*
                    true
                }

                fn filter_entities(&self, entities: &mut Vec<EntityId<E>>, context: &Context) {
                    // The fast path: If this query is indexed, we only have to do one pass over the entities.
                    if let Some(multi_property_id) = self.multi_property_id() {
                        let property_store = context.entity_store.get_property_store::<E>();
                        // The `index_unindexed_people` method returns `false` if the property is not indexed.
                        if property_store.index_unindexed_entities_for_property_id(context, multi_property_id) {
                            // Fetch the right hash bucket from the index and return it.
                            let property_value_store = property_store.get_with_id(multi_property_id);
                            if let Some(entity_set) = property_value_store.get_index_set_with_hash(
                                self.multi_property_value_hash(),
                            ) {
                                entities.retain(|entity_id| entity_set.contains(entity_id) );
                            } else {
                                // Since we already checked that this multi-property is indexed, it must be that
                                // there are no entities having this property value.
                                entities.clear();
                            }
                            return;
                        }
                        // If the property is not indexed, we fall through.
                    }

                    // The slow path: Check each property of the query separately.
                    #(
                        {
                            let property_value_store = context.get_property_value_store::<E, T~N>();
                            entities.retain(
                                |entity|{
                                    self._~N == property_value_store.get(*entity)
                                }
                            );
                        }
                    )*
                }
            }
        });
    }
}

// Import the PropertyEntityValues types for 2-10 properties
use crate::entity::query::property_entity_values::{
    PropertyEntityValues10, PropertyEntityValues2, PropertyEntityValues3, PropertyEntityValues4,
    PropertyEntityValues5, PropertyEntityValues6, PropertyEntityValues7, PropertyEntityValues8,
    PropertyEntityValues9,
};

// Implement Query for PropertyEntityValues2..10
impl_query!(PropertyEntityValues2, 2);
impl_query!(PropertyEntityValues3, 3);
impl_query!(PropertyEntityValues4, 4);
impl_query!(PropertyEntityValues5, 5);
impl_query!(PropertyEntityValues6, 6);
impl_query!(PropertyEntityValues7, 7);
impl_query!(PropertyEntityValues8, 8);
impl_query!(PropertyEntityValues9, 9);
impl_query!(PropertyEntityValues10, 10);
