mod query_impls;
mod query_result_iterator;
mod source_set;

use crate::hashing::HashMap;
use crate::people::multi_property::type_ids_to_multi_property_id;
use crate::{people::HashValueType, Context};
pub use query_result_iterator::QueryResultIterator;
use std::any::TypeId;
use std::sync::{Mutex, OnceLock};

/// Encapsulates a person query.
///
/// [`Context::query_people`] actually takes an instance of [`Query`], but because
/// we implement Query for tuples of up to size 20, that's invisible
/// to the caller. Do not use this trait directly.
pub trait Query: Copy + 'static {
    fn setup(&self, context: &Context);
    /// Returns a list of `(type_id, hash)` pairs where `hash` is the hash of a value of type
    /// `Property::Value` and `type_id` is `Property.type_id()` (NOT the type ID of the value).
    fn get_query(&self) -> Vec<(TypeId, HashValueType)>;

    /// Returns an unordered list of type IDs of the properties in this query.
    fn get_type_ids(&self) -> Vec<TypeId>;

    /// Returns the `TypeId` of the multi-property having the properties of this query, if any.
    fn multi_property_type_id(&self) -> Option<TypeId> {
        // This trick allows us to cache the multi-property ID so we don't have to allocate every
        // time.
        static REGISTRY: OnceLock<Mutex<HashMap<TypeId, &'static Option<TypeId>>>> =
            OnceLock::new();

        let map = REGISTRY.get_or_init(|| Mutex::new(HashMap::default()));
        let mut map = map.lock().unwrap();
        let type_id = TypeId::of::<Self>();
        let entry = *map.entry(type_id).or_insert_with(|| {
            let mut types = self.get_type_ids();
            types.sort_unstable();
            Box::leak(Box::new(type_ids_to_multi_property_id(types.as_slice())))
        });

        *entry
    }

    /// If this query is a multi-property query, this method computes the hash of the
    /// multi-property value.
    fn multi_property_value_hash(&self) -> HashValueType;

    /// Creates a new `QueryResultIterator`
    fn new_query_result_iterator<'c>(&self, context: &'c Context) -> QueryResultIterator<'c>;
}

#[cfg(test)]
mod tests {
    #![allow(dead_code)]
    use crate::people::PeoplePlugin;
    use crate::{
        define_derived_property, define_multi_property, define_person_property, Context,
        ContextPeopleExt, HashSetExt, PersonProperty,
    };
    use serde_derive::Serialize;

    define_person_property!(Age, u8);
    define_person_property!(County, u32);
    define_person_property!(Height, u32);

    #[derive(Serialize, Copy, Clone, PartialEq, Eq, Debug)]
    pub enum RiskCategoryValue {
        High,
        Low,
    }

    define_person_property!(RiskCategory, RiskCategoryValue);

    #[test]
    fn with_query_results() {
        let mut context = Context::new();
        let _ = context
            .add_person((RiskCategory, RiskCategoryValue::High))
            .unwrap();

        context.with_query_results((RiskCategory, RiskCategoryValue::High), &mut |people| {
            assert_eq!(people.len(), 1);
        });
    }

    #[test]
    fn with_query_results_empty() {
        let context = Context::new();

        context.with_query_results((RiskCategory, RiskCategoryValue::High), &mut |people| {
            assert_eq!(people.len(), 0);
        });
    }

    #[test]
    fn query_people_count() {
        let mut context = Context::new();
        let _ = context
            .add_person((RiskCategory, RiskCategoryValue::High))
            .unwrap();

        assert_eq!(
            context.query_people_count((RiskCategory, RiskCategoryValue::High)),
            1
        );
    }

    #[test]
    fn query_people_count_empty() {
        let context = Context::new();

        assert_eq!(
            context.query_people_count((RiskCategory, RiskCategoryValue::High)),
            0
        );
    }

    #[test]
    fn with_query_results_macro_index_first() {
        let mut context = Context::new();
        let _ = context
            .add_person((RiskCategory, RiskCategoryValue::High))
            .unwrap();
        context.index_property(RiskCategory);
        assert!(is_property_indexed::<RiskCategory>(&context));

        context.with_query_results((RiskCategory, RiskCategoryValue::High), &mut |people| {
            assert_eq!(people.len(), 1);
        });
    }

    fn is_property_indexed<T: PersonProperty>(context: &Context) -> bool {
        let container = context.get_data(PeoplePlugin);
        container
            .property_indexes
            .borrow()
            .get(&T::type_id())
            .and_then(|index| Some(index.is_indexed()))
            .unwrap_or(false)
    }

    #[test]
    fn with_query_results_macro_index_second() {
        let mut context = Context::new();
        let _ = context.add_person((RiskCategory, RiskCategoryValue::High));

        context.with_query_results((RiskCategory, RiskCategoryValue::High), &mut |people| {
            assert_eq!(people.len(), 1);
        });
        assert!(!is_property_indexed::<RiskCategory>(&context));

        context.index_property(RiskCategory);
        assert!(is_property_indexed::<RiskCategory>(&context));

        context.with_query_results((RiskCategory, RiskCategoryValue::High), &mut |people| {
            assert_eq!(people.len(), 1);
        });
    }

    #[test]
    fn with_query_results_macro_change() {
        let mut context = Context::new();
        let person1 = context
            .add_person((RiskCategory, RiskCategoryValue::High))
            .unwrap();

        context.with_query_results((RiskCategory, RiskCategoryValue::High), &mut |people| {
            assert_eq!(people.len(), 1);
        });

        context.with_query_results((RiskCategory, RiskCategoryValue::Low), &mut |people| {
            assert_eq!(people.len(), 0);
        });

        context.set_person_property(person1, RiskCategory, RiskCategoryValue::Low);
        context.with_query_results((RiskCategory, RiskCategoryValue::High), &mut |people| {
            assert_eq!(people.len(), 0);
        });

        context.with_query_results((RiskCategory, RiskCategoryValue::Low), &mut |people| {
            assert_eq!(people.len(), 1);
        });
    }

    #[test]
    fn with_query_results_index_after_add() {
        let mut context = Context::new();
        let _ = context
            .add_person((RiskCategory, RiskCategoryValue::High))
            .unwrap();
        context.index_property(RiskCategory);
        assert!(is_property_indexed::<RiskCategory>(&context));
        context.with_query_results((RiskCategory, RiskCategoryValue::High), &mut |people| {
            assert_eq!(people.len(), 1);
        });
    }

    #[test]
    fn with_query_results_add_after_index() {
        let mut context = Context::new();
        let _ = context
            .add_person((RiskCategory, RiskCategoryValue::High))
            .unwrap();
        context.index_property(RiskCategory);
        assert!(is_property_indexed::<RiskCategory>(&context));
        context.with_query_results((RiskCategory, RiskCategoryValue::High), &mut |people| {
            assert_eq!(people.len(), 1);
        });

        let _ = context
            .add_person((RiskCategory, RiskCategoryValue::High))
            .unwrap();
        context.with_query_results((RiskCategory, RiskCategoryValue::High), &mut |people| {
            assert_eq!(people.len(), 2);
        });
    }

    #[test]
    // This is safe because we reindex only when someone queries.
    fn add_after_index_without_query() {
        let mut context = Context::new();
        let _ = context.add_person(()).unwrap();
        context.index_property(RiskCategory);
    }

    #[test]
    #[should_panic(expected = "Property not initialized")]
    // This will panic when we query.
    fn with_query_results_add_after_index_panic() {
        let mut context = Context::new();
        context.add_person(()).unwrap();
        context.index_property(RiskCategory);
        context.with_query_results((RiskCategory, RiskCategoryValue::High), &mut |_people| {});
    }

    #[test]
    fn with_query_results_cast_value() {
        let mut context = Context::new();
        let _ = context.add_person((Age, 42)).unwrap();

        // Age is a u8, by default integer literals are i32; the macro should cast it.
        context.with_query_results((Age, 42), &mut |people| {
            assert_eq!(people.len(), 1);
        });
    }

    #[test]
    fn with_query_results_intersection() {
        let mut context = Context::new();
        let _ = context
            .add_person(((Age, 42), (RiskCategory, RiskCategoryValue::High)))
            .unwrap();
        let _ = context
            .add_person(((Age, 42), (RiskCategory, RiskCategoryValue::Low)))
            .unwrap();
        let _ = context
            .add_person(((Age, 40), (RiskCategory, RiskCategoryValue::Low)))
            .unwrap();

        context.with_query_results(
            ((Age, 42), (RiskCategory, RiskCategoryValue::High)),
            &mut |people| {
                assert_eq!(people.len(), 1);
            },
        );
    }

    #[test]
    fn with_query_results_intersection_non_macro() {
        let mut context = Context::new();
        let _ = context
            .add_person(((Age, 42), (RiskCategory, RiskCategoryValue::High)))
            .unwrap();
        let _ = context
            .add_person(((Age, 42), (RiskCategory, RiskCategoryValue::Low)))
            .unwrap();
        let _ = context
            .add_person(((Age, 40), (RiskCategory, RiskCategoryValue::Low)))
            .unwrap();

        context.with_query_results(
            ((Age, 42), (RiskCategory, RiskCategoryValue::High)),
            &mut |people| {
                assert_eq!(people.len(), 1);
            },
        );
    }

    #[test]
    fn with_query_results_intersection_one_indexed() {
        let mut context = Context::new();
        let _ = context
            .add_person(((Age, 42), (RiskCategory, RiskCategoryValue::High)))
            .unwrap();
        let _ = context
            .add_person(((Age, 42), (RiskCategory, RiskCategoryValue::Low)))
            .unwrap();
        let _ = context
            .add_person(((Age, 40), (RiskCategory, RiskCategoryValue::Low)))
            .unwrap();

        context.index_property(Age);
        context.with_query_results(
            ((Age, 42), (RiskCategory, RiskCategoryValue::High)),
            &mut |people| {
                assert_eq!(people.len(), 1);
            },
        );
    }

    #[test]
    fn query_derived_prop() {
        let mut context = Context::new();
        define_derived_property!(Senior, bool, [Age], |age| age >= 65);

        let person = context.add_person((Age, 64)).unwrap();
        let _ = context.add_person((Age, 88));

        let mut not_seniors = Vec::new();
        context.with_query_results((Senior, false), &mut |people| {
            not_seniors = people.to_owned_vec()
        });
        let mut seniors = Vec::new();
        context.with_query_results((Senior, true), &mut |people| {
            seniors = people.to_owned_vec();
        });
        assert_eq!(seniors.len(), 1, "One senior");
        assert_eq!(not_seniors.len(), 1, "One non-senior");

        context.set_person_property(person, Age, 65);

        context.with_query_results((Senior, false), &mut |people| {
            not_seniors = people.to_owned_vec()
        });
        context.with_query_results((Senior, true), &mut |people| {
            seniors = people.to_owned_vec()
        });

        assert_eq!(seniors.len(), 2, "Two seniors");
        assert_eq!(not_seniors.len(), 0, "No non-seniors");
    }

    #[test]
    fn query_derived_prop_with_index() {
        let mut context = Context::new();
        define_derived_property!(Senior, bool, [Age], |age| age >= 65);

        context.index_property(Senior);
        let person = context.add_person((Age, 64)).unwrap();
        let _ = context.add_person((Age, 88));

        // Age is a u8, by default integer literals are i32; the macro should cast it.
        let mut not_seniors = Vec::new();
        context.with_query_results((Senior, false), &mut |people| {
            not_seniors = people.to_owned_vec()
        });
        let mut seniors = Vec::new();
        context.with_query_results((Senior, true), &mut |people| {
            seniors = people.to_owned_vec()
        });
        assert_eq!(seniors.len(), 1, "One senior");
        assert_eq!(not_seniors.len(), 1, "One non-senior");

        context.set_person_property(person, Age, 65);

        context.with_query_results((Senior, false), &mut |people| {
            not_seniors = people.to_owned_vec()
        });
        context.with_query_results((Senior, true), &mut |people| {
            seniors = people.to_owned_vec()
        });

        assert_eq!(seniors.len(), 2, "Two seniors");
        assert_eq!(not_seniors.len(), 0, "No non-seniors");
    }

    // create a multi-property index
    define_multi_property!(ACH, (Age, County, Height));
    define_multi_property!(CH, (County, Height));

    #[test]
    fn query_derived_prop_with_optimized_index() {
        let mut context = Context::new();
        // create a 'regular' derived property
        define_derived_property!(
            Ach,
            (u8, u32, u32),
            [Age, County, Height],
            |age, county, height| { (age, county, height) }
        );

        // add some people
        let _ = context.add_person(((Age, 64), (County, 2), (Height, 120)));
        let _ = context.add_person(((Age, 88), (County, 2), (Height, 130)));
        let p2 = context
            .add_person(((Age, 8), (County, 1), (Height, 140)))
            .unwrap();
        let p3 = context
            .add_person(((Age, 28), (County, 1), (Height, 140)))
            .unwrap();
        let p4 = context
            .add_person(((Age, 28), (County, 2), (Height, 160)))
            .unwrap();
        let p5 = context
            .add_person(((Age, 28), (County, 2), (Height, 160)))
            .unwrap();

        // 'regular' derived property
        context.with_query_results((Ach, (28, 2, 160)), &mut |people| {
            assert_eq!(people.len(), 2, "Should have 2 matches");
            assert!(people.contains(&p4));
            assert!(people.contains(&p5));
        });

        // multi-property index
        context.with_query_results(((Age, 28), (County, 2), (Height, 160)), &mut |people| {
            assert_eq!(people.len(), 2, "Should have 2 matches");
            assert!(people.contains(&p4));
            assert!(people.contains(&p5));
        });

        // multi-property index with different order
        context.with_query_results(((County, 2), (Height, 160), (Age, 28)), &mut |people| {
            assert_eq!(people.len(), 2, "Should have 2 matches");
            assert!(people.contains(&p4));
            assert!(people.contains(&p5));
        });

        // multi-property index with different order
        context.with_query_results(((Height, 160), (County, 2), (Age, 28)), &mut |people| {
            assert_eq!(people.len(), 2, "Should have 2 matches");
            assert!(people.contains(&p4));
            assert!(people.contains(&p5));
        });

        // multi-property index with different order and different value
        context.with_query_results(((Height, 140), (County, 1), (Age, 28)), &mut |people| {
            assert_eq!(people.len(), 1, "Should have 1 matches");
            assert!(people.contains(&p3));
        });

        context.set_person_property(p2, Age, 28);
        // multi-property index again after changing the value
        context.with_query_results(((Height, 140), (County, 1), (Age, 28)), &mut |people| {
            assert_eq!(people.len(), 2, "Should have 2 matches");
            assert!(people.contains(&p2));
            assert!(people.contains(&p3));
        });

        context.with_query_results(((Height, 140), (County, 1)), &mut |people| {
            assert_eq!(people.len(), 2, "Should have 2 matches");
            assert!(people.contains(&p2));
            assert!(people.contains(&p3));
        });
    }
}
