use crate::hashing::{one_shot_128, HashMap};
use crate::people::multi_property::{
    static_apply_reordering, static_sorted_indices, type_ids_to_multi_property_id,
};
use crate::{people::HashValueType, Context, ContextPeopleExt, PersonProperty};
use seq_macro::seq;
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

    fn multi_property_value_hash(&self) -> HashValueType;
}

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
}

// Implement the query version with one parameter as a singleton tuple. We split this out from the
// `impl_query` macro to avoid applying the `SortedTuple` machinery to such a simple case and so
// that `multi_property_type_id()` can just return `Some(T1::type_id())`.
impl<T1: PersonProperty> Query for ((T1, T1::Value), ) {
    fn setup(&self, context: &Context) {
        context.register_property::<T1>();
    }

    fn get_query(&self) -> Vec<(TypeId, HashValueType)> {
        let value = T1::make_canonical(self.0.1);
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
        T1::hash_property_value(&T1::make_canonical(self.0.1))
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
                    let mut values: [&Vec<u8>; $ct] = [
                        #(
                            &$crate::bincode::serde::encode_to_vec(self.N.1, bincode::config::standard()).unwrap(),
                        )*
                    ];
                    let indices: [usize; $ct] = static_sorted_indices( &keys);
                    static_apply_reordering(&mut values, &indices);
                    let data = values.into_iter().flatten().copied().collect::<Vec<u8>>();
                    one_shot_128(&data.as_slice())
                }

            }
        });
    }
}

// Implement the versions with 2..10 parameters. (The 1 case is implemented above.)
seq!(Z in 2..10 {
    impl_query!(Z);
});

#[cfg(test)]
mod tests {
    #![allow(dead_code)]
    use crate::people::PeoplePlugin;
    use crate::{
        define_derived_property, define_multi_property, define_person_property, Context,
        ContextPeopleExt, PersonProperty,
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
    fn query_people() {
        let mut context = Context::new();
        let _ = context
            .add_person((RiskCategory, RiskCategoryValue::High))
            .unwrap();

        let people = context.query_people((RiskCategory, RiskCategoryValue::High));
        assert_eq!(people.len(), 1);
    }

    #[test]
    fn query_people_empty() {
        let context = Context::new();

        let people = context.query_people((RiskCategory, RiskCategoryValue::High));
        assert_eq!(people.len(), 0);
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
    fn query_people_macro_index_first() {
        let mut context = Context::new();
        let _ = context
            .add_person((RiskCategory, RiskCategoryValue::High))
            .unwrap();
        context.index_property(RiskCategory);
        assert!(is_property_indexed::<RiskCategory>(&context));
        let people = context.query_people((RiskCategory, RiskCategoryValue::High));
        assert_eq!(people.len(), 1);
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
    fn query_people_macro_index_second() {
        let mut context = Context::new();
        let _ = context.add_person((RiskCategory, RiskCategoryValue::High));
        {
            let people = context.query_people((RiskCategory, RiskCategoryValue::High));
            assert!(!is_property_indexed::<RiskCategory>(&context));
            assert_eq!(people.len(), 1);
        }
        context.index_property(RiskCategory);
        assert!(is_property_indexed::<RiskCategory>(&context));
        let people = context.query_people((RiskCategory, RiskCategoryValue::High));
        assert_eq!(people.len(), 1);
    }

    #[test]
    fn query_people_macro_change() {
        let mut context = Context::new();
        let person1 = context
            .add_person((RiskCategory, RiskCategoryValue::High))
            .unwrap();

        {
            let people = context.query_people((RiskCategory, RiskCategoryValue::High));
            assert_eq!(people.len(), 1);
            let people = context.query_people((RiskCategory, RiskCategoryValue::Low));
            assert_eq!(people.len(), 0);
        }

        context.set_person_property(person1, RiskCategory, RiskCategoryValue::Low);
        let people = context.query_people((RiskCategory, RiskCategoryValue::High));
        assert_eq!(people.len(), 0);
        let people = context.query_people((RiskCategory, RiskCategoryValue::Low));
        assert_eq!(people.len(), 1);
    }

    #[test]
    fn query_people_index_after_add() {
        let mut context = Context::new();
        let _ = context
            .add_person((RiskCategory, RiskCategoryValue::High))
            .unwrap();
        context.index_property(RiskCategory);
        assert!(is_property_indexed::<RiskCategory>(&context));
        let people = context.query_people((RiskCategory, RiskCategoryValue::High));
        assert_eq!(people.len(), 1);
    }

    #[test]
    fn query_people_add_after_index() {
        let mut context = Context::new();
        let _ = context
            .add_person((RiskCategory, RiskCategoryValue::High))
            .unwrap();
        {
            context.index_property(RiskCategory);
            assert!(is_property_indexed::<RiskCategory>(&context));
            let people = context.query_people((RiskCategory, RiskCategoryValue::High));
            assert_eq!(people.len(), 1);
        }

        let _ = context
            .add_person((RiskCategory, RiskCategoryValue::High))
            .unwrap();
        let people = context.query_people((RiskCategory, RiskCategoryValue::High));
        assert_eq!(people.len(), 2);
    }

    #[test]
    // This is safe because we reindex only when someone queries.
    fn query_people_add_after_index_without_query() {
        let mut context = Context::new();
        let _ = context.add_person(()).unwrap();
        context.index_property(RiskCategory);
    }

    #[test]
    #[should_panic(expected = "Property not initialized")]
    // This will panic when we query.
    fn query_people_add_after_index_panic() {
        let mut context = Context::new();
        context.add_person(()).unwrap();
        context.index_property(RiskCategory);
        context.query_people((RiskCategory, RiskCategoryValue::High));
    }

    #[test]
    fn query_people_cast_value() {
        let mut context = Context::new();
        let _ = context.add_person((Age, 42)).unwrap();

        // Age is a u8, by default integer literals are i32; the macro should cast it.
        let people = context.query_people((Age, 42));
        assert_eq!(people.len(), 1);
    }

    #[test]
    fn query_people_intersection() {
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

        let people = context.query_people(((Age, 42), (RiskCategory, RiskCategoryValue::High)));
        assert_eq!(people.len(), 1);
    }

    #[test]
    fn query_people_intersection_non_macro() {
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

        let people = context.query_people(((Age, 42), (RiskCategory, RiskCategoryValue::High)));
        assert_eq!(people.len(), 1);
    }

    #[test]
    fn query_people_intersection_one_indexed() {
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
        let people = context.query_people(((Age, 42), (RiskCategory, RiskCategoryValue::High)));
        assert_eq!(people.len(), 1);
    }

    #[test]
    fn query_derived_prop() {
        let mut context = Context::new();
        define_derived_property!(Senior, bool, [Age], |age| age >= 65);

        let person = context.add_person((Age, 64)).unwrap();
        let _ = context.add_person((Age, 88)).unwrap();

        {
            // Age is a u8, by default integer literals are i32; the macro should cast it.
            let not_seniors = context.query_people((Senior, false));
            let seniors = context.query_people((Senior, true));
            assert_eq!(seniors.len(), 1, "One senior");
            assert_eq!(not_seniors.len(), 1, "One non-senior");
        }

        context.set_person_property(person, Age, 65);

        let not_seniors = context.query_people((Senior, false));
        let seniors = context.query_people((Senior, true));

        assert_eq!(seniors.len(), 2, "Two seniors");
        assert_eq!(not_seniors.len(), 0, "No non-seniors");
    }

    #[test]
    fn query_derived_prop_with_index() {
        let mut context = Context::new();
        define_derived_property!(Senior, bool, [Age], |age| age >= 65);

        context.index_property(Senior);
        let person = context.add_person((Age, 64)).unwrap();
        let _ = context.add_person((Age, 88)).unwrap();
        {
            // Age is a u8, by default integer literals are i32; the macro should cast it.
            let not_seniors = context.query_people((Senior, false));
            let seniors = context.query_people((Senior, true));
            assert_eq!(seniors.len(), 1, "One senior");
            assert_eq!(not_seniors.len(), 1, "One non-senior");
        }

        context.set_person_property(person, Age, 65);

        let not_seniors = context.query_people((Senior, false));
        let seniors = context.query_people((Senior, true));

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
        let _person = context
            .add_person(((Age, 64), (County, 2), (Height, 120)))
            .unwrap();
        let _ = context
            .add_person(((Age, 88), (County, 2), (Height, 130)))
            .unwrap();
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

        {
            // 'regular' derived property
            let ach_people = context.query_people((Ach, (28, 2, 160)));
            assert_eq!(ach_people.len(), 2, "Should have 2 matches");
            assert!(ach_people.contains(&p4));
            assert!(ach_people.contains(&p5));

            // multi-property index
            let age_county_height2 = context.query_people(((Age, 28), (County, 2), (Height, 160)));
            assert_eq!(age_county_height2.len(), 2, "Should have 2 matches");
            assert!(age_county_height2.contains(&p4));
            assert!(age_county_height2.contains(&p5));

            // multi-property index with different order
            let age_county_height3 = context.query_people(((County, 2), (Height, 160), (Age, 28)));
            assert_eq!(age_county_height3.len(), 2, "Should have 2 matches");
            assert!(age_county_height3.contains(&p4));
            assert!(age_county_height3.contains(&p5));

            // multi-property index with different order
            let age_county_height4 = context.query_people(((Height, 160), (County, 2), (Age, 28)));
            assert_eq!(age_county_height4.len(), 2, "Should have 2 matches");
            assert!(age_county_height4.contains(&p4));
            assert!(age_county_height4.contains(&p5));

            // multi-property index with different order and different value
            let age_county_height5 = context.query_people(((Height, 140), (County, 1), (Age, 28)));
            assert_eq!(age_county_height5.len(), 1, "Should have 1 matches");
            assert!(age_county_height5.contains(&p3));
        }

        context.set_person_property(p2, Age, 28);
        // multi-property index again after changing the value
        let age_county_height5 = context.query_people(((Height, 140), (County, 1), (Age, 28)));
        assert_eq!(age_county_height5.len(), 2, "Should have 2 matches");
        assert!(age_county_height5.contains(&p2));
        assert!(age_county_height5.contains(&p3));

        let age_county_height5 = context.query_people(((Height, 140), (County, 1)));
        assert_eq!(age_county_height5.len(), 2, "Should have 2 matches");
        assert!(age_county_height5.contains(&p2));
        assert!(age_county_height5.contains(&p3));
    }
}
