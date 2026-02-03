mod query_impls;
mod query_result_iterator;
mod source_set;

use std::any::TypeId;
use std::sync::{Mutex, OnceLock};

pub use query_result_iterator::QueryResultIterator;

use crate::entity::multi_property::type_ids_to_multi_property_index;
use crate::entity::{Entity, HashValueType};
use crate::hashing::HashMap;
use crate::prelude::EntityId;
use crate::Context;

/// Encapsulates a query.
///
/// [`ContextEntitiesExt::query_result_iterator`](crate::entity::context_extension::ContextEntitiesExt::query_result_iterator)
/// actually takes an instance of [`Query`], but because
/// we implement Query for tuples of up to size 20, that's invisible
/// to the caller. Do not use this trait directly.
pub trait Query<E: Entity>: Copy + 'static {
    /// Returns a list of `(type_id, hash)` pairs where `hash` is the hash of the property value
    /// and `type_id` is `Property.type_id()`.
    fn get_query(&self) -> Vec<(usize, HashValueType)>;

    /// Returns an unordered list of type IDs of the properties in this query.
    fn get_type_ids(&self) -> Vec<TypeId>;

    /// Returns the `TypeId` of the multi-property having the properties of this query, if any.
    fn multi_property_id(&self) -> Option<usize> {
        // This trick allows us to cache the multi-property ID so we don't have to allocate every
        // time.
        static REGISTRY: OnceLock<Mutex<HashMap<TypeId, &'static Option<usize>>>> = OnceLock::new();

        let map = REGISTRY.get_or_init(|| Mutex::new(HashMap::default()));
        let mut map = map.lock().unwrap();
        let type_id = TypeId::of::<Self>();
        let entry = *map.entry(type_id).or_insert_with(|| {
            let mut types = self.get_type_ids();
            types.sort_unstable();
            Box::leak(Box::new(type_ids_to_multi_property_index(types.as_slice())))
        });

        *entry
    }

    /// If this query is a multi-property query, this method computes the hash of the
    /// multi-property value.
    fn multi_property_value_hash(&self) -> HashValueType;

    /// Creates a new `QueryResultIterator`
    fn new_query_result_iterator<'c>(&self, context: &'c Context) -> QueryResultIterator<'c, E>;

    /// Determines if the given person matches this query.
    fn match_entity(&self, entity_id: EntityId<E>, context: &Context) -> bool;

    /// Removes all `EntityId`s from the given vector that do not match this query.
    fn filter_entities(&self, entities: &mut Vec<EntityId<E>>, context: &Context);
}

#[cfg(test)]
mod tests {

    use crate::prelude::*;
    use crate::{
        define_derived_property, define_entity, define_multi_property, define_property, Context,
        HashSetExt,
    };

    define_entity!(Person, PersonId);

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
    fn with_query_results() {
        let mut context = Context::new();
        let _ = context.add_entity((RiskCategory::High,)).unwrap();

        context.with_query_results((RiskCategory::High,), &mut |people| {
            assert_eq!(people.len(), 1);
        });
    }

    #[test]
    fn with_query_results_empty() {
        let context = Context::new();

        context.with_query_results((RiskCategory::High,), &mut |people| {
            assert_eq!(people.len(), 0);
        });
    }

    #[test]
    fn query_entity_count() {
        let mut context = Context::new();
        let _ = context.add_entity((RiskCategory::High,)).unwrap();

        assert_eq!(context.query_entity_count((RiskCategory::High,)), 1);
    }

    #[test]
    fn query_entity_count_empty() {
        let context = Context::new();

        assert_eq!(context.query_entity_count((RiskCategory::High,)), 0);
    }

    #[test]
    fn with_query_results_macro_index_first() {
        let mut context = Context::new();
        let _ = context.add_entity((RiskCategory::High,)).unwrap();
        context.index_property::<_, RiskCategory>();
        assert!(context.is_property_indexed::<Person, RiskCategory>());

        context.with_query_results((RiskCategory::High,), &mut |people| {
            assert_eq!(people.len(), 1);
        });
    }

    #[test]
    fn with_query_results_macro_index_second() {
        let mut context = Context::new();
        let _ = context.add_entity((RiskCategory::High,));

        context.with_query_results((RiskCategory::High,), &mut |people| {
            assert_eq!(people.len(), 1);
        });
        assert!(!context.is_property_indexed::<Person, RiskCategory>());

        context.index_property::<Person, RiskCategory>();
        assert!(context.is_property_indexed::<Person, RiskCategory>());

        context.with_query_results((RiskCategory::High,), &mut |people| {
            assert_eq!(people.len(), 1);
        });
    }

    #[test]
    fn with_query_results_macro_change() {
        let mut context = Context::new();
        let person1 = context.add_entity((RiskCategory::High,)).unwrap();

        context.with_query_results((RiskCategory::High,), &mut |people| {
            assert_eq!(people.len(), 1);
        });

        context.with_query_results((RiskCategory::Low,), &mut |people| {
            assert_eq!(people.len(), 0);
        });

        context.set_property(person1, RiskCategory::Low);
        context.with_query_results((RiskCategory::High,), &mut |people| {
            assert_eq!(people.len(), 0);
        });

        context.with_query_results((RiskCategory::Low,), &mut |people| {
            assert_eq!(people.len(), 1);
        });
    }

    #[test]
    fn with_query_results_index_after_add() {
        let mut context = Context::new();
        let _ = context.add_entity((RiskCategory::High,)).unwrap();
        context.index_property::<Person, RiskCategory>();
        assert!(context.is_property_indexed::<Person, RiskCategory>());
        context.with_query_results((RiskCategory::High,), &mut |people| {
            assert_eq!(people.len(), 1);
        });
    }

    #[test]
    fn with_query_results_add_after_index() {
        let mut context = Context::new();
        let _ = context.add_entity((RiskCategory::High,)).unwrap();
        context.index_property::<Person, RiskCategory>();
        assert!(context.is_property_indexed::<Person, RiskCategory>());
        context.with_query_results((RiskCategory::High,), &mut |people| {
            assert_eq!(people.len(), 1);
        });

        let _ = context.add_entity((RiskCategory::High,)).unwrap();
        context.with_query_results((RiskCategory::High,), &mut |people| {
            assert_eq!(people.len(), 2);
        });
    }

    #[test]
    fn with_query_results_cast_value() {
        let mut context = Context::new();
        let _ = context.add_entity((Age(42), RiskCategory::High)).unwrap();

        context.with_query_results((Age(42),), &mut |people| {
            assert_eq!(people.len(), 1);
        });
    }

    #[test]
    fn with_query_results_intersection() {
        let mut context = Context::new();
        let _ = context.add_entity((Age(42), RiskCategory::High)).unwrap();
        let _ = context.add_entity((Age(42), RiskCategory::Low)).unwrap();
        let _ = context.add_entity((Age(40), RiskCategory::Low)).unwrap();

        context.with_query_results((Age(42), RiskCategory::High), &mut |people| {
            assert_eq!(people.len(), 1);
        });
    }

    #[test]
    fn with_query_results_intersection_non_macro() {
        let mut context = Context::new();
        let _ = context.add_entity((Age(42), RiskCategory::High)).unwrap();
        let _ = context.add_entity((Age(42), RiskCategory::Low)).unwrap();
        let _ = context.add_entity((Age(40), RiskCategory::Low)).unwrap();

        context.with_query_results((Age(42), RiskCategory::High), &mut |people| {
            assert_eq!(people.len(), 1);
        });
    }

    #[test]
    fn with_query_results_intersection_one_indexed() {
        let mut context = Context::new();
        let _ = context.add_entity((Age(42), RiskCategory::High)).unwrap();
        let _ = context.add_entity((Age(42), RiskCategory::Low)).unwrap();
        let _ = context.add_entity((Age(40), RiskCategory::Low)).unwrap();

        context.index_property::<Person, Age>();
        context.with_query_results((Age(42), RiskCategory::High), &mut |people| {
            assert_eq!(people.len(), 1);
        });
    }

    #[test]
    fn query_derived_prop() {
        let mut context = Context::new();
        define_derived_property!(struct Senior(bool), Person, [Age], |age| Senior(age.0 >= 65));

        let person = context.add_entity((Age(64), RiskCategory::High)).unwrap();
        context.add_entity((Age(88), RiskCategory::High)).unwrap();

        let mut not_seniors = Vec::new();
        context.with_query_results((Senior(false),), &mut |people| {
            not_seniors = people.to_owned_vec()
        });
        let mut seniors = Vec::new();
        context.with_query_results((Senior(true),), &mut |people| {
            seniors = people.to_owned_vec();
        });
        assert_eq!(seniors.len(), 1, "One senior");
        assert_eq!(not_seniors.len(), 1, "One non-senior");

        context.set_property(person, Age(65));

        context.with_query_results((Senior(false),), &mut |people| {
            not_seniors = people.to_owned_vec()
        });
        context.with_query_results((Senior(true),), &mut |people| {
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
        let person = context.add_entity((Age(64), RiskCategory::Low)).unwrap();
        let _ = context.add_entity((Age(88), RiskCategory::Low));

        let mut not_seniors = Vec::new();
        context.with_query_results((Senior(false),), &mut |people| {
            not_seniors = people.to_owned_vec()
        });
        let mut seniors = Vec::new();
        context.with_query_results((Senior(true),), &mut |people| {
            seniors = people.to_owned_vec()
        });
        assert_eq!(seniors.len(), 1, "One senior");
        assert_eq!(not_seniors.len(), 1, "One non-senior");

        context.set_property(person, Age(65));

        context.with_query_results((Senior(false),), &mut |people| {
            not_seniors = people.to_owned_vec()
        });
        context.with_query_results((Senior(true),), &mut |people| {
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
        let _ = context.add_entity((Age(64), County(2), Height(120), RiskCategory::Low));
        let _ = context.add_entity((Age(88), County(2), Height(130), RiskCategory::Low));
        let p2 = context
            .add_entity((Age(8), County(1), Height(140), RiskCategory::Low))
            .unwrap();
        let p3 = context
            .add_entity((Age(28), County(1), Height(140), RiskCategory::Low))
            .unwrap();
        let p4 = context
            .add_entity((Age(28), County(2), Height(160), RiskCategory::Low))
            .unwrap();
        let p5 = context
            .add_entity((Age(28), County(2), Height(160), RiskCategory::Low))
            .unwrap();

        // 'regular' derived property
        context.with_query_results((Ach(28, 2, 160),), &mut |people| {
            assert_eq!(people.len(), 2, "Should have 2 matches");
            assert!(people.contains(&p4));
            assert!(people.contains(&p5));
        });

        // multi-property index
        context.with_query_results((Age(28), County(2), Height(160)), &mut |people| {
            assert_eq!(people.len(), 2, "Should have 2 matches");
            assert!(people.contains(&p4));
            assert!(people.contains(&p5));
        });

        // multi-property index with different order
        context.with_query_results((County(2), Height(160), Age(28)), &mut |people| {
            assert_eq!(people.len(), 2, "Should have 2 matches");
            assert!(people.contains(&p4));
            assert!(people.contains(&p5));
        });

        // multi-property index with different order
        context.with_query_results((Height(160), County(2), Age(28)), &mut |people| {
            assert_eq!(people.len(), 2, "Should have 2 matches");
            assert!(people.contains(&p4));
            assert!(people.contains(&p5));
        });

        // multi-property index with different order and different value
        context.with_query_results((Height(140), County(1), Age(28)), &mut |people| {
            assert_eq!(people.len(), 1, "Should have 1 matches");
            assert!(people.contains(&p3));
        });

        context.set_property(p2, Age(28));
        // multi-property index again after changing the value
        context.with_query_results((Height(140), County(1), Age(28)), &mut |people| {
            assert_eq!(people.len(), 2, "Should have 2 matches");
            assert!(people.contains(&p2));
            assert!(people.contains(&p3));
        });

        context.with_query_results((Height(140), County(1)), &mut |people| {
            assert_eq!(people.len(), 2, "Should have 2 matches");
            assert!(people.contains(&p2));
            assert!(people.contains(&p3));
        });
    }

    #[test]
    fn test_match_entity() {
        let mut context = Context::new();
        let person = context
            .add_entity((Age(28), County(2), Height(160), RiskCategory::Low))
            .unwrap();
        assert!(context.match_entity(person, (Age(28), County(2), Height(160))));
        assert!(!context.match_entity(person, (Age(13), County(2), Height(160))));
        assert!(!context.match_entity(person, (Age(28), County(33), Height(160))));
        assert!(!context.match_entity(person, (Age(28), County(2), Height(9))));
    }

    #[test]
    fn filter_entities_for_unindexed_query() {
        let mut context = Context::new();
        let mut people = Vec::new();

        for idx in 0..10 {
            let person = context
                .add_entity((Age(28), County(idx % 2), Height(160), RiskCategory::Low))
                .unwrap();
            people.push(person);
        }

        context.filter_entities(
            &mut people,
            (Age(28), County(0), Height(160), RiskCategory::Low),
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
                .add_entity((Age(28), County(idx % 2), Height(160), RiskCategory::Low))
                .unwrap();
            people.push(person);
        }

        context.filter_entities(&mut people, (County(0), Age(28)));

        let expected = (0..5)
            .map(|idx| PersonId::new(idx * 2))
            .collect::<Vec<PersonId>>();
        assert_eq!(people, expected);
    }
}
