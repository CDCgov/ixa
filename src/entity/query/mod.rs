mod query_impls;

use std::any::TypeId;
use std::marker::PhantomData;
use std::sync::{Mutex, OnceLock};

use crate::entity::entity_set::{EntitySet, EntitySetIterator};
use crate::entity::multi_property::type_ids_to_multi_property_id;
use crate::entity::property_list::{PropertyInitializationList, PropertyList};
use crate::entity::property_store::PropertyStore;
use crate::entity::Entity;
use crate::hashing::HashMap;
use crate::prelude::EntityId;
use crate::{Context, IxaError};

/// A newtype wrapper that associates a tuple of property values with an entity type.
///
/// This is not meant to be used directly, but rather as a backing for the with! macro/
/// a replacement for the query tuple.
///
/// # Example
/// ```ignore
/// use ixa::{define_entity, define_property, with};
///
/// define_entity!(Person);
/// define_property!(struct Age(u8), Person, default_const = Age(0));
///
/// // Build a query for people with Age(42).
/// let query = with!(Person, Age(42));
/// ```
pub struct EntityPropertyTuple<E: Entity, T> {
    inner: T,
    _marker: PhantomData<E>,
}

// Manual implementations to avoid requiring E: Copy/Clone
impl<E: Entity, T: Copy> Copy for EntityPropertyTuple<E, T> {}

impl<E: Entity, T: Clone> Clone for EntityPropertyTuple<E, T> {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
            _marker: PhantomData,
        }
    }
}

impl<E: Entity, T: std::fmt::Debug> std::fmt::Debug for EntityPropertyTuple<E, T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EntityPropertyTuple")
            .field("inner", &self.inner)
            .finish()
    }
}

impl<E: Entity, T> EntityPropertyTuple<E, T> {
    /// Create a new `EntityPropertyTuple` wrapping the given tuple.
    pub fn new(inner: T) -> Self {
        Self {
            inner,
            _marker: PhantomData,
        }
    }

    /// Returns a reference to the inner tuple.
    pub fn inner(&self) -> &T {
        &self.inner
    }

    /// Consumes self and returns the inner tuple.
    pub fn into_inner(self) -> T {
        self.inner
    }
}

impl<E: Entity, T: QueryInternal<E>> QueryInternal<E> for EntityPropertyTuple<E, T> {
    type QueryParts<'a>
        = T::QueryParts<'a>
    where
        Self: 'a;

    fn get_type_ids(&self) -> Vec<TypeId> {
        self.inner.get_type_ids()
    }

    fn multi_property_id(&self) -> Option<usize> {
        self.inner.multi_property_id()
    }

    fn is_empty_query(&self) -> bool {
        self.inner.is_empty_query()
    }

    fn query_parts(&self) -> Self::QueryParts<'_> {
        self.inner.query_parts()
    }

    fn new_query_result<'c>(&self, context: &'c Context) -> EntitySet<'c, E> {
        self.inner.new_query_result(context)
    }

    fn match_entity(&self, entity_id: EntityId<E>, context: &Context) -> bool {
        self.inner.match_entity(entity_id, context)
    }

    fn filter_entities(&self, entities: &mut Vec<EntityId<E>>, context: &Context) {
        self.inner.filter_entities(entities, context)
    }
}

impl<E: Entity, T: PropertyList<E>> PropertyList<E> for EntityPropertyTuple<E, T> {
    fn validate() -> Result<(), IxaError> {
        T::validate()
    }

    fn contains_properties(property_type_ids: &[TypeId]) -> bool {
        T::contains_properties(property_type_ids)
    }

    fn set_values_for_new_entity(
        &self,
        entity_id: EntityId<E>,
        property_store: &mut PropertyStore<E>,
    ) {
        let tuple = *self;
        tuple
            .into_inner()
            .set_values_for_new_entity(entity_id, property_store)
    }

    fn get_values_for_entity(context: &Context, entity_id: EntityId<E>) -> Self {
        EntityPropertyTuple::new(T::get_values_for_entity(context, entity_id))
    }
}

impl<E: Entity, PL: PropertyList<E>> PropertyInitializationList<E> for EntityPropertyTuple<E, PL> {}

/// Internal query machinery.
pub trait QueryInternal<E: Entity>: 'static {
    /// Allocation-free representation of the query parts exposed by this query.
    type QueryParts<'a>: AsRef<[&'a dyn std::any::Any]>
    where
        Self: 'a;

    /// Returns an unordered list of type IDs of the properties in this query.
    #[must_use]
    fn get_type_ids(&self) -> Vec<TypeId>;

    /// Returns the property ID of the representative multi-property having the properties of
    /// this query, if any.
    #[must_use]
    fn multi_property_id(&self) -> Option<usize> {
        // Silence type complexity warning for this one-off data structure.
        #[allow(clippy::type_complexity)]
        static REGISTRY: OnceLock<Mutex<HashMap<(usize, TypeId), &'static Option<usize>>>> =
            OnceLock::new();

        let map = REGISTRY.get_or_init(|| Mutex::new(HashMap::default()));
        let mut map = map.lock().unwrap();
        let key = (E::id(), TypeId::of::<Self>());
        let entry = *map.entry(key).or_insert_with(|| {
            let mut types = self.get_type_ids();
            types.sort_unstable();
            Box::leak(Box::new(type_ids_to_multi_property_id(
                E::id(),
                types.as_slice(),
            )))
        });

        *entry
    }

    /// Indicates whether this query matches the entire population for `E`.
    #[must_use]
    fn is_empty_query(&self) -> bool {
        false
    }

    /// Exposes the query parts without allocating.
    #[must_use]
    fn query_parts(&self) -> Self::QueryParts<'_>;

    /// Creates a new query result as an `EntitySet`.
    #[must_use]
    fn new_query_result<'c>(&self, context: &'c Context) -> EntitySet<'c, E>;

    /// Creates a new `EntitySetIterator`.
    #[must_use]
    fn new_query_result_iterator<'c>(&self, context: &'c Context) -> EntitySetIterator<'c, E> {
        self.new_query_result(context).into_iter()
    }

    /// Determines if the given person matches this query.
    #[must_use]
    fn match_entity(&self, entity_id: EntityId<E>, context: &Context) -> bool;

    /// Removes all `EntityId`s from the given vector that do not match this query.
    fn filter_entities(&self, entities: &mut Vec<EntityId<E>>, context: &Context);
}

/// Values accepted by user-facing query APIs such as
/// [`ContextEntitiesExt::query`](crate::entity::context_extension::ContextEntitiesExt::query)
/// and
/// [`ContextEntitiesExt::sample_entity`](crate::entity::context_extension::ContextEntitiesExt::sample_entity).
///
/// Use [`with!`](crate::with) to query for specific property values, or pass the entity type
/// directly to work with the entire population.
pub trait Query<E: Entity>: QueryInternal<E> {}

impl<E: Entity, QI: QueryInternal<E>> Query<E> for EntityPropertyTuple<E, QI> {}
impl<E: Entity> Query<E> for E {}

#[cfg(test)]
mod tests {

    use super::QueryInternal;
    use crate::prelude::*;
    use crate::{
        define_derived_property, define_entity, define_multi_property, define_property, Context,
    };

    define_entity!(Person);

    define_property!(struct Age(u8), Person, default_const = Age(0));
    define_property!(struct County(u32), Person, default_const = County(0));
    define_property!(struct Height(u32), Person, default_const = Height(0));
    define_property!(
        enum RiskCategory {
            High,
            Low,
        },
        Person
    );

    define_multi_property!((Age, County), Person);

    #[test]
    fn empty_tuple_query_internal_matches_all_entities() {
        let mut context = Context::new();
        let person1 = context
            .add_entity(with!(Person, Age(42), RiskCategory::High))
            .unwrap();
        let person2 = context
            .add_entity(with!(Person, Age(30), RiskCategory::Low))
            .unwrap();

        assert_eq!(<() as QueryInternal<Person>>::get_type_ids(&()), Vec::new());
        assert!(<() as QueryInternal<Person>>::is_empty_query(&()));
        assert!(<() as QueryInternal<Person>>::query_parts(&())
            .as_ref()
            .is_empty());

        let people = <() as QueryInternal<Person>>::new_query_result_iterator(&(), &context)
            .collect::<Vec<_>>();
        assert_eq!(people, vec![person1, person2]);
        assert!(<() as QueryInternal<Person>>::match_entity(
            &(),
            person1,
            &context
        ));

        let mut ids = vec![person1, person2];
        <() as QueryInternal<Person>>::filter_entities(&(), &mut ids, &context);
        assert_eq!(ids, vec![person1, person2]);
    }

    #[test]
    fn entity_query_internal_has_no_type_ids() {
        let mut context = Context::new();
        let person1 = context
            .add_entity(with!(Person, Age(42), RiskCategory::High))
            .unwrap();
        let person2 = context
            .add_entity(with!(Person, Age(30), RiskCategory::Low))
            .unwrap();

        assert_eq!(
            <Person as QueryInternal<Person>>::get_type_ids(&Person),
            Vec::new()
        );
        assert_eq!(
            <Person as QueryInternal<Person>>::multi_property_id(&Person),
            None
        );
        assert!(<Person as QueryInternal<Person>>::query_parts(&Person)
            .as_ref()
            .is_empty());

        let people = <Person as QueryInternal<Person>>::new_query_result(&Person, &context)
            .into_iter()
            .collect::<Vec<_>>();
        assert_eq!(people, vec![person1, person2]);
        assert!(<Person as QueryInternal<Person>>::match_entity(
            &Person, person1, &context
        ));

        let mut ids = vec![person1, person2];
        <Person as QueryInternal<Person>>::filter_entities(&Person, &mut ids, &context);
        assert_eq!(ids, vec![person1, person2]);
    }

    #[test]
    fn singleton_query_result_iterator_uses_indexed_and_unindexed_paths() {
        let mut context = Context::new();
        let high = context
            .add_entity(with!(Person, RiskCategory::High))
            .unwrap();
        let _low = context
            .add_entity(with!(Person, RiskCategory::Low))
            .unwrap();

        let people = <(RiskCategory,) as QueryInternal<Person>>::new_query_result_iterator(
            &(RiskCategory::High,),
            &context,
        )
        .collect::<Vec<_>>();
        assert_eq!(people, vec![high]);

        let mut indexed_context = Context::new();
        indexed_context.index_property::<Person, RiskCategory>();
        let indexed_high = indexed_context
            .add_entity(with!(Person, RiskCategory::High))
            .unwrap();
        let _indexed_low = indexed_context
            .add_entity(with!(Person, RiskCategory::Low))
            .unwrap();

        let people = <(RiskCategory,) as QueryInternal<Person>>::new_query_result_iterator(
            &(RiskCategory::High,),
            &indexed_context,
        )
        .collect::<Vec<_>>();
        assert_eq!(people, vec![indexed_high]);

        let mut empty_index_context = Context::new();
        empty_index_context.index_property::<Person, Age>();
        let _ = empty_index_context
            .add_entity(with!(Person, Age(42), RiskCategory::Low))
            .unwrap();

        let people = <(Age,) as QueryInternal<Person>>::new_query_result_iterator(
            &(Age(99),),
            &empty_index_context,
        )
        .collect::<Vec<_>>();
        assert!(people.is_empty());
    }

    #[test]
    fn tuple_query_result_iterator_uses_indexed_and_unindexed_paths() {
        let mut context = Context::new();
        let matching1 = context
            .add_entity(with!(Person, Age(28), County(0), RiskCategory::High))
            .unwrap();
        let _wrong_county = context
            .add_entity(with!(Person, Age(28), County(1), RiskCategory::Low))
            .unwrap();
        let _wrong_age = context
            .add_entity(with!(Person, Age(30), County(0), RiskCategory::Low))
            .unwrap();
        let matching2 = context
            .add_entity(with!(Person, Age(28), County(0), RiskCategory::Low))
            .unwrap();

        let people = <(Age, County) as QueryInternal<Person>>::new_query_result_iterator(
            &(Age(28), County(0)),
            &context,
        )
        .collect::<Vec<_>>();
        assert_eq!(people, vec![matching1, matching2]);

        let mut indexed_context = Context::new();
        indexed_context.index_property::<Person, (Age, County)>();
        let indexed_matching = indexed_context
            .add_entity(with!(Person, Age(28), County(0), RiskCategory::High))
            .unwrap();
        let _indexed_nonmatching = indexed_context
            .add_entity(with!(Person, Age(28), County(1), RiskCategory::Low))
            .unwrap();

        let people = <(Age, County) as QueryInternal<Person>>::new_query_result_iterator(
            &(Age(28), County(0)),
            &indexed_context,
        )
        .collect::<Vec<_>>();
        assert_eq!(people, vec![indexed_matching]);

        let people = <(Age, County) as QueryInternal<Person>>::new_query_result_iterator(
            &(Age(99), County(99)),
            &indexed_context,
        )
        .collect::<Vec<_>>();
        assert!(people.is_empty());
    }

    #[test]
    fn singleton_filter_entities_keeps_matching_entities() {
        let mut context = Context::new();
        let high1 = context
            .add_entity(with!(Person, RiskCategory::High))
            .unwrap();
        let low = context
            .add_entity(with!(Person, RiskCategory::Low))
            .unwrap();
        let high2 = context
            .add_entity(with!(Person, RiskCategory::High))
            .unwrap();

        let mut people = vec![high1, low, high2];
        context.filter_entities(&mut people, with!(Person, RiskCategory::High));

        assert_eq!(people, vec![high1, high2]);
    }

    #[test]
    fn tuple_filter_entities_falls_through_after_unsupported_multi_index_lookup() {
        let mut context = Context::new();
        let matching1 = context
            .add_entity(with!(Person, Age(28), County(0), RiskCategory::High))
            .unwrap();
        let wrong_county = context
            .add_entity(with!(Person, Age(28), County(1), RiskCategory::Low))
            .unwrap();
        let wrong_age = context
            .add_entity(with!(Person, Age(30), County(0), RiskCategory::Low))
            .unwrap();
        let matching2 = context
            .add_entity(with!(Person, Age(28), County(0), RiskCategory::Low))
            .unwrap();

        let mut people = vec![matching1, wrong_county, wrong_age, matching2];
        context.filter_entities(&mut people, with!(Person, County(0), Age(28)));

        assert_eq!(people, vec![matching1, matching2]);
    }

    #[test]
    fn with_query_results() {
        let mut context = Context::new();
        let _ = context
            .add_entity(with!(Person, RiskCategory::High))
            .unwrap();

        context.with_query_results(with!(Person, RiskCategory::High), &mut |people| {
            assert_eq!(people.into_iter().count(), 1);
        });
    }

    #[test]
    fn with_query_results_empty() {
        let context = Context::new();

        context.with_query_results(with!(Person, RiskCategory::High), &mut |people| {
            assert_eq!(people.into_iter().count(), 0);
        });
    }

    #[test]
    fn query_entity_count() {
        let mut context = Context::new();
        let _ = context
            .add_entity(with!(Person, RiskCategory::High))
            .unwrap();

        assert_eq!(
            context.query_entity_count(with!(Person, RiskCategory::High)),
            1
        );
    }

    #[test]
    fn query_entity_count_empty() {
        let context = Context::new();

        assert_eq!(
            context.query_entity_count(with!(Person, RiskCategory::High)),
            0
        );
    }

    #[test]
    fn with_query_results_macro_index_first() {
        let mut context = Context::new();
        let _ = context
            .add_entity(with!(Person, RiskCategory::High))
            .unwrap();
        context.index_property::<_, RiskCategory>();
        assert!(context.is_property_indexed::<Person, RiskCategory>());

        context.with_query_results(with!(Person, RiskCategory::High), &mut |people| {
            assert_eq!(people.into_iter().count(), 1);
        });
    }

    #[test]
    fn with_query_results_macro_index_second() {
        let mut context = Context::new();
        let _ = context.add_entity(with!(Person, RiskCategory::High));

        context.with_query_results(with!(Person, RiskCategory::High), &mut |people| {
            assert_eq!(people.into_iter().count(), 1);
        });
        assert!(!context.is_property_indexed::<Person, RiskCategory>());

        context.index_property::<Person, RiskCategory>();
        assert!(context.is_property_indexed::<Person, RiskCategory>());

        context.with_query_results(with!(Person, RiskCategory::High), &mut |people| {
            assert_eq!(people.into_iter().count(), 1);
        });
    }

    #[test]
    fn with_query_results_macro_change() {
        let mut context = Context::new();
        let person1 = context
            .add_entity(with!(Person, RiskCategory::High))
            .unwrap();

        context.with_query_results(with!(Person, RiskCategory::High), &mut |people| {
            assert_eq!(people.into_iter().count(), 1);
        });

        context.with_query_results(with!(Person, RiskCategory::Low), &mut |people| {
            assert_eq!(people.into_iter().count(), 0);
        });

        context.set_property(person1, RiskCategory::Low);
        context.with_query_results(with!(Person, RiskCategory::High), &mut |people| {
            assert_eq!(people.into_iter().count(), 0);
        });

        context.with_query_results(with!(Person, RiskCategory::Low), &mut |people| {
            assert_eq!(people.into_iter().count(), 1);
        });
    }

    #[test]
    fn with_query_results_index_after_add() {
        let mut context = Context::new();
        let _ = context
            .add_entity(with!(Person, RiskCategory::High))
            .unwrap();
        context.index_property::<Person, RiskCategory>();
        assert!(context.is_property_indexed::<Person, RiskCategory>());
        context.with_query_results(with!(Person, RiskCategory::High), &mut |people| {
            assert_eq!(people.into_iter().count(), 1);
        });
    }

    #[test]
    fn with_query_results_add_after_index() {
        let mut context = Context::new();
        let _ = context
            .add_entity(with!(Person, RiskCategory::High))
            .unwrap();
        context.index_property::<Person, RiskCategory>();
        assert!(context.is_property_indexed::<Person, RiskCategory>());
        context.with_query_results(with!(Person, RiskCategory::High), &mut |people| {
            assert_eq!(people.into_iter().count(), 1);
        });

        let _ = context
            .add_entity(with!(Person, RiskCategory::High))
            .unwrap();
        context.with_query_results(with!(Person, RiskCategory::High), &mut |people| {
            assert_eq!(people.into_iter().count(), 2);
        });
    }

    #[test]
    fn with_query_results_cast_value() {
        let mut context = Context::new();
        let _ = context
            .add_entity(with!(Person, Age(42), RiskCategory::High))
            .unwrap();

        context.with_query_results(with!(Person, Age(42)), &mut |people| {
            assert_eq!(people.into_iter().count(), 1);
        });
    }

    #[test]
    fn with_query_results_intersection() {
        let mut context = Context::new();
        let _ = context
            .add_entity(with!(Person, Age(42), RiskCategory::High))
            .unwrap();
        let _ = context
            .add_entity(with!(Person, Age(42), RiskCategory::Low))
            .unwrap();
        let _ = context
            .add_entity(with!(Person, Age(40), RiskCategory::Low))
            .unwrap();

        context.with_query_results(with!(Person, Age(42), RiskCategory::High), &mut |people| {
            assert_eq!(people.into_iter().count(), 1);
        });
    }

    #[test]
    fn with_query_results_intersection_non_macro() {
        let mut context = Context::new();
        let _ = context
            .add_entity(with!(Person, Age(42), RiskCategory::High))
            .unwrap();
        let _ = context
            .add_entity(with!(Person, Age(42), RiskCategory::Low))
            .unwrap();
        let _ = context
            .add_entity(with!(Person, Age(40), RiskCategory::Low))
            .unwrap();

        context.with_query_results(with!(Person, Age(42), RiskCategory::High), &mut |people| {
            assert_eq!(people.into_iter().count(), 1);
        });
    }

    #[test]
    fn with_query_results_intersection_one_indexed() {
        let mut context = Context::new();
        let _ = context
            .add_entity(with!(Person, Age(42), RiskCategory::High))
            .unwrap();
        let _ = context
            .add_entity(with!(Person, Age(42), RiskCategory::Low))
            .unwrap();
        let _ = context
            .add_entity(with!(Person, Age(40), RiskCategory::Low))
            .unwrap();

        context.index_property::<Person, Age>();
        context.with_query_results(with!(Person, Age(42), RiskCategory::High), &mut |people| {
            assert_eq!(people.into_iter().count(), 1);
        });
    }

    #[test]
    fn query_derived_prop() {
        let mut context = Context::new();
        define_derived_property!(struct Senior(bool), Person, [Age], |age| Senior(age.0 >= 65));

        let person = context
            .add_entity(with!(Person, Age(64), RiskCategory::High))
            .unwrap();
        context
            .add_entity(with!(Person, Age(88), RiskCategory::High))
            .unwrap();

        let mut not_seniors = Vec::new();
        context.with_query_results(with!(Person, Senior(false)), &mut |people| {
            not_seniors = people.to_owned_vec();
        });
        let mut seniors = Vec::new();
        context.with_query_results(with!(Person, Senior(true)), &mut |people| {
            seniors = people.to_owned_vec();
        });
        assert_eq!(seniors.len(), 1, "One senior");
        assert_eq!(not_seniors.len(), 1, "One non-senior");

        context.set_property(person, Age(65));

        context.with_query_results(with!(Person, Senior(false)), &mut |people| {
            not_seniors = people.to_owned_vec()
        });
        context.with_query_results(with!(Person, Senior(true)), &mut |people| {
            seniors = people.to_owned_vec()
        });

        assert_eq!(seniors.len(), 2, "Two seniors");
        assert_eq!(not_seniors.len(), 0, "No non-seniors");
    }

    #[test]
    fn query_derived_prop_with_index() {
        let mut context = Context::new();
        define_derived_property!(struct Senior(bool), Person, [Age], |age| Senior(age.0 >= 65));

        context.index_property::<Person, Senior>();
        let person = context
            .add_entity(with!(Person, Age(64), RiskCategory::Low))
            .unwrap();
        let _ = context.add_entity(with!(Person, Age(88), RiskCategory::Low));

        let mut not_seniors = Vec::new();
        context.with_query_results(with!(Person, Senior(false)), &mut |people| {
            not_seniors = people.to_owned_vec()
        });
        let mut seniors = Vec::new();
        context.with_query_results(with!(Person, Senior(true)), &mut |people| {
            seniors = people.to_owned_vec()
        });
        assert_eq!(seniors.len(), 1, "One senior");
        assert_eq!(not_seniors.len(), 1, "One non-senior");

        context.set_property(person, Age(65));

        context.with_query_results(with!(Person, Senior(false)), &mut |people| {
            not_seniors = people.to_owned_vec()
        });
        context.with_query_results(with!(Person, Senior(true)), &mut |people| {
            seniors = people.to_owned_vec()
        });

        assert_eq!(seniors.len(), 2, "Two seniors");
        assert_eq!(not_seniors.len(), 0, "No non-seniors");
    }

    // create a multi-property index
    define_multi_property!((Age, County, Height), Person);
    define_multi_property!((County, Height), Person);

    #[test]
    fn query_derived_prop_with_optimized_index() {
        let mut context = Context::new();
        // create a 'regular' derived property
        define_derived_property!(
            struct Ach(u8, u32, u32),
            Person,
            [Age, County, Height],
            [],
            |age, county, height| Ach(age.0, county.0, height.0)
        );

        // add some people
        let _ = context.add_entity(with!(
            Person,
            Age(64),
            County(2),
            Height(120),
            RiskCategory::Low
        ));
        let _ = context.add_entity(with!(
            Person,
            Age(88),
            County(2),
            Height(130),
            RiskCategory::Low
        ));
        let p2 = context
            .add_entity(with!(
                Person,
                Age(8),
                County(1),
                Height(140),
                RiskCategory::Low
            ))
            .unwrap();
        let p3 = context
            .add_entity(with!(
                Person,
                Age(28),
                County(1),
                Height(140),
                RiskCategory::Low
            ))
            .unwrap();
        let p4 = context
            .add_entity(with!(
                Person,
                Age(28),
                County(2),
                Height(160),
                RiskCategory::Low
            ))
            .unwrap();
        let p5 = context
            .add_entity(with!(
                Person,
                Age(28),
                County(2),
                Height(160),
                RiskCategory::Low
            ))
            .unwrap();

        // 'regular' derived property
        context.with_query_results(with!(Person, Ach(28, 2, 160)), &mut |people| {
            assert!(people.contains(p4));
            assert!(people.contains(p5));
            assert_eq!(people.into_iter().count(), 2, "Should have 2 matches");
        });

        // multi-property index
        context.with_query_results(
            with!(Person, Age(28), County(2), Height(160)),
            &mut |people| {
                assert!(people.contains(p4));
                assert!(people.contains(p5));
                assert_eq!(people.into_iter().count(), 2, "Should have 2 matches");
            },
        );

        // multi-property index with different order
        context.with_query_results(
            with!(Person, County(2), Height(160), Age(28)),
            &mut |people| {
                assert!(people.contains(p4));
                assert!(people.contains(p5));
                assert_eq!(people.into_iter().count(), 2, "Should have 2 matches");
            },
        );

        // multi-property index with different order
        context.with_query_results(
            with!(Person, Height(160), County(2), Age(28)),
            &mut |people| {
                assert!(people.contains(p4));
                assert!(people.contains(p5));
                assert_eq!(people.into_iter().count(), 2, "Should have 2 matches");
            },
        );

        // multi-property index with different order and different value
        context.with_query_results(
            with!(Person, Height(140), County(1), Age(28)),
            &mut |people| {
                assert!(people.contains(p3));
                assert_eq!(people.into_iter().count(), 1, "Should have 1 matches");
            },
        );

        context.set_property(p2, Age(28));
        // multi-property index again after changing the value
        context.with_query_results(
            with!(Person, Height(140), County(1), Age(28)),
            &mut |people| {
                assert!(people.contains(p2));
                assert!(people.contains(p3));
                assert_eq!(people.into_iter().count(), 2, "Should have 2 matches");
            },
        );

        context.with_query_results(with!(Person, Height(140), County(1)), &mut |people| {
            assert!(people.contains(p2));
            assert!(people.contains(p3));
            assert_eq!(people.into_iter().count(), 2, "Should have 2 matches");
        });
    }

    #[test]
    fn test_match_entity() {
        let mut context = Context::new();
        let person = context
            .add_entity(with!(
                Person,
                Age(28),
                County(2),
                Height(160),
                RiskCategory::Low
            ))
            .unwrap();
        assert!(context.match_entity(person, with!(Person, Age(28), County(2), Height(160))));
        assert!(!context.match_entity(person, with!(Person, Age(13), County(2), Height(160))));
        assert!(!context.match_entity(person, with!(Person, Age(28), County(33), Height(160))));
        assert!(!context.match_entity(person, with!(Person, Age(28), County(2), Height(9))));
    }

    #[test]
    fn filter_entities_for_unindexed_query() {
        let mut context = Context::new();
        let mut people = Vec::new();

        for idx in 0..10 {
            let person = context
                .add_entity(with!(
                    Person,
                    Age(28),
                    County(idx % 2),
                    Height(160),
                    RiskCategory::Low
                ))
                .unwrap();
            people.push(person);
        }

        context.filter_entities(
            &mut people,
            with!(Person, Age(28), County(0), Height(160), RiskCategory::Low),
        );

        let expected = (0..5)
            .map(|idx| PersonId::new(idx * 2))
            .collect::<Vec<PersonId>>();
        assert_eq!(people, expected);
    }

    #[test]
    fn filter_entities_for_indexed_query() {
        let mut context = Context::new();
        let mut people = Vec::new();

        context.index_property::<Person, (Age, County)>();

        for idx in 0..10 {
            let person = context
                .add_entity(with!(
                    Person,
                    Age(28),
                    County(idx % 2),
                    Height(160),
                    RiskCategory::Low
                ))
                .unwrap();
            people.push(person);
        }

        context.filter_entities(&mut people, with!(Person, County(0), Age(28)));

        let expected = (0..5)
            .map(|idx| PersonId::new(idx * 2))
            .collect::<Vec<PersonId>>();
        assert_eq!(people, expected);
    }

    #[test]
    fn entity_property_tuple_basic() {
        use super::EntityPropertyTuple;

        let mut context = Context::new();
        let p1 = context
            .add_entity(with!(Person, Age(42), RiskCategory::High))
            .unwrap();
        let _ = context
            .add_entity(with!(Person, Age(42), RiskCategory::Low))
            .unwrap();
        let _ = context
            .add_entity(with!(Person, Age(30), RiskCategory::High))
            .unwrap();

        // Create query using EntityPropertyTuple
        let query: EntityPropertyTuple<Person, _> =
            EntityPropertyTuple::new((Age(42), RiskCategory::High));

        context.with_query_results(query, &mut |people| {
            assert!(people.contains(p1));
            assert_eq!(people.into_iter().count(), 1);
        });

        // Test match_entity
        assert!(context.match_entity(p1, query));

        // Test query_entity_count
        assert_eq!(context.query_entity_count(query), 1);
    }

    #[test]
    fn entity_property_tuple_empty_query() {
        use super::EntityPropertyTuple;

        let mut context = Context::new();
        let _ = context
            .add_entity(with!(Person, Age(42), RiskCategory::High))
            .unwrap();
        let _ = context
            .add_entity(with!(Person, Age(30), RiskCategory::Low))
            .unwrap();

        // Empty query matches all entities
        let query: EntityPropertyTuple<Person, _> = EntityPropertyTuple::new(());

        assert_eq!(context.query_entity_count(query), 2);
    }

    #[test]
    fn entity_property_tuple_singleton() {
        use super::EntityPropertyTuple;

        let mut context = Context::new();
        let _ = context
            .add_entity(with!(Person, Age(42), RiskCategory::High))
            .unwrap();
        let _ = context
            .add_entity(with!(Person, Age(42), RiskCategory::Low))
            .unwrap();
        let _ = context
            .add_entity(with!(Person, Age(30), RiskCategory::High))
            .unwrap();

        // Single property query
        let query: EntityPropertyTuple<Person, _> = EntityPropertyTuple::new((Age(42),));

        assert_eq!(context.query_entity_count(query), 2);
    }

    #[test]
    fn entity_property_tuple_inner_access() {
        use super::EntityPropertyTuple;

        let query: EntityPropertyTuple<Person, _> =
            EntityPropertyTuple::new((Age(42), RiskCategory::High));

        // Test inner() accessor
        let inner = query.inner();
        assert_eq!(inner.0, Age(42));
        assert_eq!(inner.1, RiskCategory::High);

        // Test into_inner()
        let (age, risk) = query.into_inner();
        assert_eq!(age, Age(42));
        assert_eq!(risk, RiskCategory::High);
    }

    #[test]
    fn all_macro_no_properties() {
        use crate::with;

        let mut context = Context::new();
        let _ = context
            .add_entity(with!(Person, Age(42), RiskCategory::High))
            .unwrap();
        let _ = context
            .add_entity(with!(Person, Age(30), RiskCategory::Low))
            .unwrap();

        // with!(Person) should match all Person entities
        let query = with!(Person);
        assert_eq!(context.query_entity_count(query), 2);
    }

    #[test]
    fn all_macro_single_property() {
        use crate::with;

        let mut context = Context::new();
        let _ = context
            .add_entity(with!(Person, Age(42), RiskCategory::High))
            .unwrap();
        let _ = context
            .add_entity(with!(Person, Age(42), RiskCategory::Low))
            .unwrap();
        let _ = context
            .add_entity(with!(Person, Age(30), RiskCategory::High))
            .unwrap();

        // with!(Person, Age(42)) should match entities with Age = 42
        let query = with!(Person, Age(42));
        assert_eq!(context.query_entity_count(query), 2);
    }

    #[test]
    fn all_macro_multiple_properties() {
        use crate::with;

        let mut context = Context::new();
        let p1 = context
            .add_entity(with!(Person, Age(42), RiskCategory::High))
            .unwrap();
        let _ = context
            .add_entity(with!(Person, Age(42), RiskCategory::Low))
            .unwrap();
        let _ = context
            .add_entity(with!(Person, Age(30), RiskCategory::High))
            .unwrap();

        // with!(Person, Age(42), RiskCategory::High) should match one entity
        let query = with!(Person, Age(42), RiskCategory::High);
        assert_eq!(context.query_entity_count(query), 1);

        context.with_query_results(query, &mut |people| {
            assert!(people.contains(p1));
        });
    }

    #[test]
    fn all_macro_with_trailing_comma() {
        use crate::with;

        let mut context = Context::new();
        let _ = context
            .add_entity(with!(Person, Age(42), RiskCategory::High))
            .unwrap();

        // Trailing comma should work
        let query = with!(Person, Age(42));
        assert_eq!(context.query_entity_count(query), 1);

        let query = with!(Person, Age(42), RiskCategory::High);
        assert_eq!(context.query_entity_count(query), 1);
    }

    #[test]
    fn entity_property_tuple_as_property_list() {
        use super::EntityPropertyTuple;
        use crate::entity::property_list::PropertyList;

        // Test validate
        assert!(EntityPropertyTuple::<Person, (Age,)>::validate().is_ok());
        assert!(EntityPropertyTuple::<Person, (Age, RiskCategory)>::validate().is_ok());

        // Test contains_properties
        assert!(EntityPropertyTuple::<Person, (Age,)>::contains_properties(
            &[Age::type_id()]
        ));
        assert!(
            EntityPropertyTuple::<Person, (Age, RiskCategory)>::contains_properties(&[
                Age::type_id()
            ])
        );
        assert!(
            EntityPropertyTuple::<Person, (Age, RiskCategory)>::contains_properties(&[
                Age::type_id(),
                RiskCategory::type_id()
            ])
        );
    }

    #[test]
    fn all_macro_as_property_list_for_add_entity() {
        use crate::with;

        let mut context = Context::new();

        // Use with! macro result to add an entity
        let props = with!(Person, Age(42), RiskCategory::High);
        let person = context.add_entity(props).unwrap();

        // Verify the entity was created with the correct properties
        assert_eq!(context.get_property::<Person, Age>(person), Age(42));
        assert_eq!(
            context.get_property::<Person, RiskCategory>(person),
            RiskCategory::High
        );
    }
}
