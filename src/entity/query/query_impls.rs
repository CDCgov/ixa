use std::any::TypeId;

use seq_macro::seq;

use crate::entity::multi_property::static_reorder_by_keys;
use crate::entity::property::Property;
use crate::entity::query::query_result_iterator::QueryResultIterator;
use crate::entity::query::source_set::SourceSet;
use crate::entity::{Entity, HashValueType, Query};
use crate::hashing::one_shot_128;
use crate::Context;

impl<E: Entity> Query<E> for () {
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
}

// ToDo(RobertJacobsonCDC): The following is a fundamental limitation in Rust. If downstream code *can* implement a
//     trait impl that will cause conflicting implementations with some blanket impl, it disallows it, regardless of
//     whether the conflict actually exists.
// Implement the query version with one parameter.
/*
impl<E: Entity, P1: Property<E>> Query<E> for P1 {
    fn get_query(&self) -> Vec<(usize, HashValueType)> {
        let value = P1::make_canonical(*self);
        vec![(P1::id(), P1::hash_property_value(&value))]
    }

    fn get_type_ids(&self) -> Vec<TypeId> {
        vec![P1::type_id()]
    }

    fn multi_property_id(&self) -> Option<usize> {
        // While not a "true" multi-property, it is convenient to have this method return the
        // `TypeId` of the singleton property.
        Some(P1::index_id())
    }

    fn multi_property_value_hash(&self) -> HashValueType {
        P1::hash_property_value(&P1::make_canonical(*self))
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

        if let Some(source_set) = SourceSet::new::<P1>(*self, context) {
            sources.push(source_set);
        } else {
            // If a single source set is empty, the intersection of all sources is empty.
            return QueryResultIterator::empty();
        }

        QueryResultIterator::from_sources(sources)
    }
}
*/

// Implement the query version with one parameter.
impl<E: Entity, P1: Property<E>> Query<E> for (P1,) {
    fn get_query(&self) -> Vec<(usize, HashValueType)> {
        let value = P1::make_canonical(self.0);
        vec![(P1::id(), P1::hash_property_value(&value))]
    }

    fn get_type_ids(&self) -> Vec<TypeId> {
        vec![P1::type_id()]
    }

    fn multi_property_id(&self) -> Option<usize> {
        // While not a "true" multi-property, it is convenient to have this method return the
        // `TypeId` of the singleton property.
        Some(P1::index_id())
    }

    fn multi_property_value_hash(&self) -> HashValueType {
        P1::hash_property_value(&P1::make_canonical(self.0))
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

        if let Some(source_set) = SourceSet::new::<P1>(self.0, context) {
            sources.push(source_set);
        } else {
            // If a single source set is empty, the intersection of all sources is empty.
            return QueryResultIterator::empty();
        }

        QueryResultIterator::from_sources(sources)
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
            > Query<E> for (
                #(
                    T~N,
                )*
            )
            {
                fn get_query(&self) -> Vec<(usize, HashValueType)> {
                    let mut ordered_items = vec![
                    #(
                        (T~N::id(), T~N::hash_property_value(&T~N::make_canonical(self.N))),
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
                            &$crate::bincode::serde::encode_to_vec(self.N, bincode::config::standard()).unwrap(),
                        )*
                    ];
                    static_reorder_by_keys(&keys, &mut values);

                    let data = values.into_iter().flatten().copied().collect::<Vec<u8>>();
                    one_shot_128(&data.as_slice())
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
                            if let Some(people_set) = property_value_store.get_index_set_with_hash(
                                self.multi_property_value_hash(),
                            ) {
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

                    #(
                        if let Some(source_set) = SourceSet::new::<T~N>(self.N, context) {
                            sources.push(source_set);
                        } else {
                            // If a single source set is empty, the intersection of all sources is empty.
                            return QueryResultIterator::empty();
                        }
                    )*

                    QueryResultIterator::from_sources(sources)
                }
            }
        });
    }
}

// Implement the versions with 2..10 parameters. (The 1 case is implemented above.)
seq!(Z in 2..10 {
    impl_query!(Z);
});
