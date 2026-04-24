use std::any::TypeId;

use seq_macro::seq;

use crate::entity::entity_set::{EntitySet, EntitySetIterator, SourceSet};
use crate::entity::index::IndexSetResult;
use crate::entity::property::Property;
use crate::entity::query::QueryInternal;
use crate::entity::{ContextEntitiesExt, Entity, EntityId};
use crate::Context;

impl<E: Entity> QueryInternal<E> for () {
    type QueryParts<'a>
        = [&'a dyn std::any::Any; 0]
    where
        Self: 'a;

    fn get_type_ids(&self) -> Vec<TypeId> {
        Vec::new()
    }

    fn multi_property_id(&self) -> Option<usize> {
        None
    }

    fn query_parts(&self) -> Self::QueryParts<'_> {
        []
    }

    fn new_query_result<'c>(&self, context: &'c Context) -> EntitySet<'c, E> {
        EntitySet::from_source(SourceSet::Population(context.get_entity_count::<E>()))
    }

    fn new_query_result_iterator<'c>(&self, context: &'c Context) -> EntitySetIterator<'c, E> {
        EntitySetIterator::from_population_iterator(context.get_entity_iterator::<E>())
    }

    fn match_entity(&self, _entity_id: EntityId<E>, _context: &Context) -> bool {
        // Every entity matches the empty query.
        true
    }

    fn filter_entities(&self, _entities: &mut Vec<EntityId<E>>, _context: &Context) {
        // Nothing to do.
    }
}

// An Entity ZST itself is an empty query matching all entities of that type.
// This allows `context.sample_entity(Rng, Person)` instead of `context.sample_entity(Rng, ())`.
impl<E: Entity> QueryInternal<E> for E {
    type QueryParts<'a>
        = [&'a dyn std::any::Any; 0]
    where
        Self: 'a;

    fn get_type_ids(&self) -> Vec<TypeId> {
        Vec::new()
    }

    fn multi_property_id(&self) -> Option<usize> {
        None
    }

    fn query_parts(&self) -> Self::QueryParts<'_> {
        []
    }

    fn new_query_result<'c>(&self, context: &'c Context) -> EntitySet<'c, E> {
        EntitySet::from_source(SourceSet::Population(context.get_entity_count::<E>()))
    }

    fn new_query_result_iterator<'c>(&self, context: &'c Context) -> EntitySetIterator<'c, E> {
        let population_iterator = context.get_entity_iterator::<E>();
        EntitySetIterator::from_population_iterator(population_iterator)
    }

    fn match_entity(&self, _entity_id: EntityId<E>, _context: &Context) -> bool {
        true
    }

    fn filter_entities(&self, _entities: &mut Vec<EntityId<E>>, _context: &Context) {
        // Nothing to do.
    }
}

// Implement the query version with one parameter.
impl<E: Entity, P1: Property<E>> QueryInternal<E> for (P1,) {
    type QueryParts<'a>
        = P1::QueryParts<'a>
    where
        Self: 'a;

    fn get_type_ids(&self) -> Vec<TypeId> {
        vec![P1::type_id()]
    }

    fn multi_property_id(&self) -> Option<usize> {
        // While not a "true" multi-property, it is convenient to have this method return the
        // `TypeId` of the singleton property.
        Some(P1::index_id())
    }

    fn query_parts(&self) -> Self::QueryParts<'_> {
        P1::query_parts_for_value(&self.0)
    }

    fn new_query_result<'c>(&self, context: &'c Context) -> EntitySet<'c, E> {
        let property_store = context.entity_store.get_property_store::<E>();

        // The case of an indexed multi-property.
        // This mirrors the indexed case in `SourceSet<'a, E>::new()`. The difference is, if the
        // multi-property is unindexed, we fall through to create `SourceSet`s for the components
        // rather than wrapping a `DerivedPropertySource`.
        if let Some(multi_property_id) = self.multi_property_id() {
            let query_parts = P1::query_parts_for_value(&self.0);
            let lookup_result = property_store
                .get_index_set_for_query_parts(multi_property_id, query_parts.as_ref());
            match lookup_result {
                IndexSetResult::Set(people_set) => {
                    return EntitySet::from_source(SourceSet::IndexSet(people_set));
                }
                IndexSetResult::Empty => {
                    return EntitySet::empty();
                }
                IndexSetResult::Unsupported => {}
            }
            // If the property is not indexed, we fall through.
        }

        // We create a source set for each property.
        let mut sources: Vec<SourceSet<E>> = Vec::new();

        if let Some(source_set) = SourceSet::new::<P1>(self.0, context) {
            sources.push(source_set);
        } else {
            // If a single source set is empty, the intersection of all sources is empty.
            return EntitySet::empty();
        }

        EntitySet::from_intersection_sources(sources)
    }

    fn new_query_result_iterator<'c>(&self, context: &'c Context) -> EntitySetIterator<'c, E> {
        // Constructing the `EntitySetIterator` directly instead of constructing an `EntitySet`
        // first is a micro-optimization improving tight-loop benchmark performance.
        let property_store = context.entity_store.get_property_store::<E>();

        if let Some(multi_property_id) = self.multi_property_id() {
            let query_parts = P1::query_parts_for_value(&self.0);
            let lookup_result = property_store
                .get_index_set_for_query_parts(multi_property_id, query_parts.as_ref());
            match lookup_result {
                IndexSetResult::Set(people_set) => {
                    return EntitySetIterator::from_index_set(people_set);
                }
                IndexSetResult::Empty => {
                    return EntitySetIterator::empty();
                }
                IndexSetResult::Unsupported => {}
            }
        }

        let mut sources: Vec<SourceSet<E>> = Vec::new();

        if let Some(source_set) = SourceSet::new::<P1>(self.0, context) {
            sources.push(source_set);
        } else {
            return EntitySetIterator::empty();
        }

        EntitySetIterator::from_sources(sources)
    }

    fn match_entity(&self, entity_id: EntityId<E>, context: &Context) -> bool {
        let found_value: P1 = context.get_property(entity_id);
        found_value == self.0
    }

    fn filter_entities(&self, entities: &mut Vec<EntityId<E>>, context: &Context) {
        let property_value_store = context.get_property_value_store::<E, P1>();
        entities.retain(|entity| self.0 == property_value_store.get(*entity));
    }
}

macro_rules! impl_query {
    ($ct:expr) => {
        seq!(N in 0..$ct {
            impl<
                E: Entity,
                #(
                    T~N : Property<E>,
                )*
            > QueryInternal<E> for (
                #(
                    T~N,
                )*
            )
            {
                type QueryParts<'a> = [&'a dyn std::any::Any; $ct] where Self: 'a;

                fn get_type_ids(&self) -> Vec<TypeId> {
                    vec![
                        #(
                            <T~N as $crate::entity::property::Property<E>>::type_id(),
                        )*
                    ]
                }

                fn query_parts(&self) -> Self::QueryParts<'_> {
                    let keys = [
                        #(
                            <T~N as $crate::entity::property::Property<E>>::name(),
                        )*
                    ];
                    let mut query_parts = [
                        #(
                            &self.N as &dyn std::any::Any,
                        )*
                    ];
                    $crate::entity::multi_property::static_reorder_by_keys(&keys, &mut query_parts);
                    query_parts
                }

                fn new_query_result<'c>(&self, context: &'c Context) -> EntitySet<'c, E> {
                    // The case of an indexed multi-property.
                    // This mirrors the indexed case in `SourceSet<'a, E>::new()`. The difference is, if the
                    // multi-property is unindexed, we fall through to create `SourceSet`s for the components
                    // rather than wrapping a `DerivedPropertySource`.
                    if let Some(multi_property_id) = <Self as $crate::entity::QueryInternal<E>>::multi_property_id(self) {
                        let property_store = context.entity_store.get_property_store::<E>();
                        let query_parts = <Self as $crate::entity::QueryInternal<E>>::query_parts(self);
                        let lookup_result = property_store.get_index_set_for_query_parts(
                            multi_property_id,
                            query_parts.as_ref(),
                        );
                        match lookup_result {
                            $crate::entity::index::IndexSetResult::Set(entity_set) => {
                                return EntitySet::from_source(SourceSet::IndexSet(entity_set));
                            }
                            $crate::entity::index::IndexSetResult::Empty => {
                                return EntitySet::empty();
                            }
                            $crate::entity::index::IndexSetResult::Unsupported => {}
                        }
                        // If the property is not indexed, we fall through.
                    }

                    // We create a source set for each property.
                    let mut sources: Vec<SourceSet<E>> = Vec::new();

                    #(
                        if let Some(source_set) = SourceSet::new::<T~N>(self.N, context) {
                            sources.push(source_set);
                        } else {
                            // If a single source set is empty, the intersection of all sources is empty.
                            return EntitySet::empty();
                        }
                    )*

                    EntitySet::from_intersection_sources(sources)
                }

                fn new_query_result_iterator<'c>(&self, context: &'c Context) -> EntitySetIterator<'c, E> {
                    // Constructing the `EntitySetIterator` directly instead of constructing an `EntitySet`
                    // first is a micro-optimization improving tight-loop benchmark performance.
                    if let Some(multi_property_id) = <Self as $crate::entity::QueryInternal<E>>::multi_property_id(self) {
                        let property_store = context.entity_store.get_property_store::<E>();
                        let query_parts = <Self as $crate::entity::QueryInternal<E>>::query_parts(self);
                        let lookup_result = property_store.get_index_set_for_query_parts(
                            multi_property_id,
                            query_parts.as_ref(),
                        );
                        match lookup_result {
                            $crate::entity::index::IndexSetResult::Set(entity_set) => {
                                return EntitySetIterator::from_index_set(entity_set);
                            }
                            $crate::entity::index::IndexSetResult::Empty => {
                                return EntitySetIterator::empty();
                            }
                            $crate::entity::index::IndexSetResult::Unsupported => {}
                        }
                    }

                    let mut sources: Vec<SourceSet<E>> = Vec::new();

                    #(
                        if let Some(source_set) = SourceSet::new::<T~N>(self.N, context) {
                            sources.push(source_set);
                        } else {
                            return EntitySetIterator::empty();
                        }
                    )*

                    EntitySetIterator::from_sources(sources)
                }

                fn match_entity(&self, entity_id: EntityId<E>, context: &Context) -> bool {
                    #(
                        {
                            let found_value: T~N = context.get_property(entity_id);
                            if found_value != self.N {
                                return false
                            }
                        }
                    )*
                    true
                }

                fn filter_entities(&self, entities: &mut Vec<EntityId<E>>, context: &Context) {
                    // The fast path: If this query is indexed, we only have to do one pass over the entities.
                    if let Some(multi_property_id) = <Self as $crate::entity::QueryInternal<E>>::multi_property_id(self) {
                        let property_store = context.entity_store.get_property_store::<E>();
                        let query_parts = <Self as $crate::entity::QueryInternal<E>>::query_parts(self);
                        let lookup_result = property_store.get_index_set_for_query_parts(
                            multi_property_id,
                            query_parts.as_ref(),
                        );
                        match lookup_result {
                            $crate::entity::index::IndexSetResult::Set(entity_set) => {
                                entities.retain(|entity_id| entity_set.contains(entity_id));
                                return;
                            }
                            $crate::entity::index::IndexSetResult::Empty => {
                                entities.clear();
                                return;
                            }
                            $crate::entity::index::IndexSetResult::Unsupported => {}
                        }
                        // If the property is not indexed, we fall through.
                    }

                    // The slow path: Check each property of the query separately.
                    #(
                        {
                            let property_value_store = context.get_property_value_store::<E, T~N>();
                            entities.retain(
                                |entity|{
                                    self.N == property_value_store.get(*entity)
                                }
                            );
                        }
                    )*
                }
            }
        });
    }
}

// Implement the versions with 2..20 parameters. (The 0 and 1 case are implemented above.)
seq!(Z in 2..20 {
    impl_query!(Z);
});
