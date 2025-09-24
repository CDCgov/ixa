use crate::hashing::one_shot_128;
use crate::people::data::PeopleData;
use crate::people::query::query_result_iterator::QueryResultIterator;
use crate::people::query::source_set::{ConcretePropertyVec, SourceSet};
use crate::people::{static_reorder_by_keys, PeoplePlugin};
use crate::people::{HashValueType, Query};
use crate::{Context, ContextPeopleExt, PersonProperty};
use seq_macro::seq;
use std::any::TypeId;
use std::cell::Ref;

impl Query for () {
    fn setup(&self, _: &Context) {}

    fn get_query(&self) -> Vec<(TypeId, HashValueType)> {
        Vec::new()
    }

    fn get_type_ids(&self) -> Vec<TypeId> {
        Vec::new()
    }

    fn multi_property_type_id(&self) -> Option<TypeId> {
        None
    }

    fn multi_property_value_hash(&self) -> HashValueType {
        let empty: &[u128] = &[];
        one_shot_128(&empty)
    }

    fn new_query_result_iterator<'c>(&self, context: &'c Context) -> QueryResultIterator<'c> {
        let container = context.get_data(PeoplePlugin);
        QueryResultIterator::from_population_iterator(container.people_iterator())
    }
}

// Implement the query version with one parameter.
impl<T1: PersonProperty> Query for (T1, T1::Value) {
    fn setup(&self, context: &Context) {
        context.register_property::<T1>();
    }

    fn get_query(&self) -> Vec<(TypeId, HashValueType)> {
        let value = T1::make_canonical(self.1);
        vec![(T1::type_id(), T1::hash_property_value(&value))]
    }

    fn get_type_ids(&self) -> Vec<TypeId> {
        vec![T1::type_id()]
    }

    fn multi_property_type_id(&self) -> Option<TypeId> {
        // While not a "true" multi-property, it is convenient to have this method return the
        // `TypeId` of the singleton property.
        Some(T1::type_id())
    }

    fn multi_property_value_hash(&self) -> HashValueType {
        T1::hash_property_value(&T1::make_canonical(self.1))
    }

    fn new_query_result_iterator<'c>(&self, context: &'c Context) -> QueryResultIterator<'c> {
        self.setup(context); // Q::setup(&query, context);
        let data_container = context.get_data(PeoplePlugin);

        // The case of a single indexed property or an indexed multi-property
        if let Some(multi_property_id) = self.multi_property_type_id() {
            // The `index_unindexed_people` method returns `false` if the property is not indexed.
            if data_container.index_unindexed_people_for_type_id(context, multi_property_id) {
                if let Some(people_set) = data_container.get_index_set_for_hash_type_id(
                    multi_property_id,
                    self.multi_property_value_hash(),
                ) {
                    return QueryResultIterator::from_index_set(people_set);
                }
            }
        }

        // This iterator says whether each property is indexed or not. There is exactly one entry
        // per property in this query. This saves us from having to do the lookup for index
        // sets that don't exist for unindexed properties. Indexes need to be updated before
        // we hold immutable references to index sets, as it's obviously a mutating operation.
        let mut is_indexed =
            [data_container.index_unindexed_people_for_type_id(context, T1::type_id())].into_iter();

        // The case of at least one unindexed property, possibly with other indexed or unindexed properties.
        // We create a source set for each property.
        let mut sources: Vec<SourceSet> = Vec::new();

        if let Some(source_set) =
            get_source_set::<T1>(self.1, is_indexed.next().unwrap(), data_container)
        {
            sources.push(source_set);
        } else {
            return QueryResultIterator::empty();
        }

        QueryResultIterator::from_sources(sources)
    }
}

// Implement the query version with one parameter as a singleton tuple. We split this out from the
// `impl_query` macro to avoid applying the `SortedTuple` machinery to such a simple case and so
// that `multi_property_type_id()` can just return `Some(T1::type_id())`.
impl<T1: PersonProperty> Query for ((T1, T1::Value),) {
    fn setup(&self, context: &Context) {
        (self.0 .0, self.0 .1).setup(context);
    }

    fn get_query(&self) -> Vec<(TypeId, HashValueType)> {
        (self.0 .0, self.0 .1).get_query()
    }

    fn get_type_ids(&self) -> Vec<TypeId> {
        (self.0 .0, self.0 .1).get_type_ids()
    }

    fn multi_property_type_id(&self) -> Option<TypeId> {
        // While not a "true" multi-property, it is convenient to have this method return the
        // `TypeId` of the singleton property.
        Some(T1::type_id())
    }

    fn multi_property_value_hash(&self) -> HashValueType {
        (self.0 .0, self.0 .1).multi_property_value_hash()
    }

    fn new_query_result_iterator<'c>(&self, context: &'c Context) -> QueryResultIterator<'c> {
        (self.0 .0, self.0 .1).new_query_result_iterator(context)
    }
}

/// A helper function that factors out common code for creating `SourceSet`s during construction of
/// `QueryResultIterator`.
///
/// We first look for an index set and if not found, we then fetch the property backing vector.
fn get_source_set<'c, P: PersonProperty>(
    value: P::Value,
    is_indexed: bool,
    data_container: &'c PeopleData,
) -> Option<SourceSet<'c>> {
    if is_indexed {
        // The property is indexed, but there is no index set for the value, and so there are
        // no query results.
        data_container
            .get_index_set::<P>(value)
            .map(SourceSet::IndexSet)
    } else {
        // In the unindexed case, we fetch the backing vector for the property
        if P::is_derived() {
            panic!("cannot query on unindexed derived property {}", P::name());
        }

        let values = Ref::filter_map(data_container.properties_map.borrow(), |index_map| {
            if let Some(stored_property_values) = index_map.get(&P::type_id()) {
                // The following `unwrap` is safe, because only `T::Value` stores are associated to `T::type_id`.
                let values: &Vec<Option<P::Value>> =
                    stored_property_values.values.downcast_ref().unwrap();
                Some(values)
            } else {
                None
            }
        })
        .ok()
        .unwrap_or_else(|| panic!("property vec not found for type {}", P::name()));

        Some(SourceSet::<'c>::PropertyVec(Box::<
            ConcretePropertyVec<'c, P>,
        >::new(
            ConcretePropertyVec::<'c, P>::new(values, value),
        )))
    }
}

macro_rules! impl_query {
    ($ct:expr) => {
        seq!(N in 0..$ct {
            impl<
                #(
                    T~N : PersonProperty,
                )*
            > Query for (
                #(
                    (T~N, T~N::Value),
                )*
            )
            {
                fn setup(&self, context: &Context) {
                    #(
                        context.register_property::<T~N>();
                    )*
                }

                fn get_query(&self) -> Vec<(TypeId, HashValueType)> {
                    let mut ordered_items = vec![
                    #(
                        (T~N::type_id(), T~N::hash_property_value(&T~N::make_canonical(self.N.1))),
                    )*
                    ];
                    ordered_items.sort_by(|a, b| a.0.cmp(&b.0));
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
                    // using staticly allocated arrays.

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
                            &$crate::bincode::serde::encode_to_vec(self.N.1, bincode::config::standard()).unwrap(),
                        )*
                    ];
                    static_reorder_by_keys(&keys, &mut values);

                    let data = values.into_iter().flatten().copied().collect::<Vec<u8>>();
                    one_shot_128(&data.as_slice())
                }

                fn new_query_result_iterator<'c>(&self, context: &'c Context) -> QueryResultIterator<'c> {
                    self.setup(context); // Q::setup(&query, context);
                    let data_container = context.get_data(PeoplePlugin);

                    // The case of a single indexed property or an indexed multi-property
                    if let Some(multi_property_id) = self.multi_property_type_id() {
                        // The `index_unindexed_people` method returns `false` if the property is not indexed.
                        if data_container.index_unindexed_people_for_type_id(context, multi_property_id) {
                            if let Some(people_set) = data_container.get_index_set_for_hash_type_id(
                                multi_property_id,
                                self.multi_property_value_hash(),
                            ) {
                                return QueryResultIterator::from_index_set(people_set);
                            }
                        }
                    }

                    // This iterator says whether each property is indexed or not. There is exactly one entry
                    // per property in this query. This saves us from having to do the lookup for index
                    // sets that don't exist for unindexed properties. Indexes need to be updated before
                    // we hold immutable references to index sets, as it's obviously a mutating operation.
                    let mut is_indexed =
                        [
                            #(
                                data_container.index_unindexed_people_for_type_id(context, T~N::type_id()),
                            )*
                        ].into_iter();

                    // The case of at least one unindexed property, possibly with other indexed or unindexed properties.
                    // We create a source set for each property.
                    let mut sources: Vec<SourceSet> = Vec::new();

                    #(
                        if let Some(source_set) =
                            get_source_set::<T~N>(self.N.1, is_indexed.next().unwrap(), data_container)
                        {
                            sources.push(source_set);
                        } else {
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
