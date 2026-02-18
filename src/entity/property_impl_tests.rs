// The macro for this is found in ixa-macros/property_impl.rs

// We define unused properties to test macro implementation.
#![allow(dead_code)]

use ixa::entity::Query;
use ixa::prelude::*;
use serde::Serialize;

define_entity!(Person);
define_entity!(Group);

define_property!(struct Pu32(u32), Person, default_const = Pu32(0));
define_property!(struct POu32(Option<u32>), Person, default_const = POu32(None));
define_property!(struct Name(&'static str), Person, default_const = Name(""));
define_property!(struct Age(u8), Person, default_const = Age(0));
#[derive(Debug, PartialEq, Clone, Copy, serde::Serialize, serde::Deserialize)]
struct Weight(f64);
impl_property!(Weight, Person, default_const = Weight(0.0));

// A struct with named fields
#[derive(Debug, PartialEq, Clone, Copy, serde::Serialize, serde::Deserialize)]
struct Innocculation {
    time: f64,
    dose: u8,
}
impl_property!(
    Innocculation,
    Person,
    default_const = Innocculation { time: 0.0, dose: 0 }
);

// An enum non-derived property
define_property!(
    enum InfectionStatus {
        Susceptible,
        Infected,
        Recovered,
    },
    Person,
    default_const = InfectionStatus::Susceptible
);

// An enum derived property
define_derived_property!(
    enum AgeGroup {
        Child,
        Adult,
        Senior,
    },
    Person,
    [Age], // Depends only on age
    |age| {
        let age: Age = age;
        if age.0 < 18 {
            AgeGroup::Child
        } else if age.0 < 65 {
            AgeGroup::Adult
        } else {
            AgeGroup::Senior
        }
    }
);

// Derived property - computed from other properties
define_derived_property!(struct DerivedProp(bool), Person, [Age],
    |age| {
        DerivedProp(age.0 % 2 == 0)
    }
);

// A property type for two distinct entities.
#[derive(Debug, PartialEq, Clone, Copy, Serialize)]
pub enum InfectionKind {
    Respiratory,
    Genetic,
    Superficial,
}
impl_property!(
    InfectionKind,
    Person,
    default_const = InfectionKind::Respiratory
);
impl_property!(InfectionKind, Group, default_const = InfectionKind::Genetic);

define_multi_property!((Name, Age, Weight), Person);
define_multi_property!((Age, Weight, Name), Person);
define_multi_property!((Weight, Age, Name), Person);

// For convenience
type ProfileNAW = (Name, Age, Weight);
type ProfileAWN = (Age, Weight, Name);
type ProfileWAN = (Weight, Age, Name);

#[test]
fn test_multi_property_ordering() {
    let a = (Name("Jane"), Age(22), Weight(180.5));
    let b = (Age(22), Weight(180.5), Name("Jane"));
    let c = (Weight(180.5), Age(22), Name("Jane"));

    // Multi-properties share the same index
    assert_eq!(ProfileNAW::index_id(), ProfileAWN::index_id());
    assert_eq!(ProfileNAW::index_id(), ProfileWAN::index_id());

    let a_canonical: <ProfileNAW as Property<_>>::CanonicalValue = ProfileNAW::make_canonical(a);
    let b_canonical: <ProfileAWN as Property<_>>::CanonicalValue = ProfileAWN::make_canonical(b);
    let c_canonical: <ProfileWAN as Property<_>>::CanonicalValue = ProfileWAN::make_canonical(c);

    assert_eq!(a_canonical, b_canonical);
    assert_eq!(a_canonical, c_canonical);

    // Actually, all of the `Profile***::hash_property_value` methods should be the same,
    // so we could use any single one.
    assert_eq!(
        ProfileNAW::hash_property_value(&a_canonical),
        ProfileAWN::hash_property_value(&b_canonical)
    );
    assert_eq!(
        ProfileNAW::hash_property_value(&a_canonical),
        ProfileWAN::hash_property_value(&c_canonical)
    );

    // Since the canonical values are the same, we could have used any single one, but this
    // demonstrates that we can convert from one order to another.
    assert_eq!(ProfileNAW::make_uncanonical(b_canonical), a);
    assert_eq!(ProfileAWN::make_uncanonical(c_canonical), b);
    assert_eq!(ProfileWAN::make_uncanonical(a_canonical), c);
}

#[test]
fn test_multi_property_vs_property_query() {
    let mut context = Context::new();

    context
        .add_entity((Name("John"), Age(42), Weight(220.5)))
        .unwrap();
    context
        .add_entity((Name("Jane"), Age(22), Weight(180.5)))
        .unwrap();
    context
        .add_entity((Name("Bob"), Age(32), Weight(190.5)))
        .unwrap();
    context
        .add_entity((Name("Alice"), Age(22), Weight(170.5)))
        .unwrap();

    context.index_property::<_, ProfileNAW>();

    // Check that all equivalent multi-properties are indexed...
    assert!(context.is_property_indexed::<Person, ProfileNAW>());
    assert!(context.is_property_indexed::<Person, ProfileAWN>());
    assert!(context.is_property_indexed::<Person, ProfileWAN>());
    // ...but only one `Index<E, P>` instance was created.
    let mut indexed_count = 0;
    if context
        .get_property_value_store::<Person, ProfileNAW>()
        .index_type()
        != ixa::entity::index::PropertyIndexType::Unindexed
    {
        indexed_count += 1;
    }
    if context
        .get_property_value_store::<Person, ProfileAWN>()
        .index_type()
        != ixa::entity::index::PropertyIndexType::Unindexed
    {
        indexed_count += 1;
    }
    if context
        .get_property_value_store::<Person, ProfileWAN>()
        .index_type()
        != ixa::entity::index::PropertyIndexType::Unindexed
    {
        indexed_count += 1;
    }
    assert_eq!(indexed_count, 1);

    {
        let example_query = (Name("Alice"), Age(22), Weight(170.5));
        let query_multi_property_id =
            <(Name, Age, Weight) as Query<Person>>::multi_property_id(&example_query);
        assert!(query_multi_property_id.is_some());
        assert_eq!(ProfileNAW::index_id(), query_multi_property_id.unwrap());
        assert_eq!(
            Query::multi_property_value_hash(&example_query),
            ProfileNAW::hash_property_value(
                &(Name("Alice"), Age(22), Weight(170.5)).make_canonical()
            )
        );
    }

    context.with_query_results(((Name("John"), Age(42), Weight(220.5)),), &mut |results| {
        assert_eq!(results.len(), 1);
    });
}

#[test]
fn test_derived_property() {
    let mut context = Context::new();

    let senior = context.add_entity::<Person, _>((Age(92),)).unwrap();
    let child = context.add_entity::<Person, _>((Age(12),)).unwrap();
    let adult = context.add_entity::<Person, _>((Age(44),)).unwrap();

    let senior_group: AgeGroup = context.get_property(senior);
    let child_group: AgeGroup = context.get_property(child);
    let adult_group: AgeGroup = context.get_property(adult);

    assert_eq!(senior_group, AgeGroup::Senior);
    assert_eq!(child_group, AgeGroup::Child);
    assert_eq!(adult_group, AgeGroup::Adult);

    // Age has no dependencies (only dependents)
    assert!(Age::non_derived_dependencies().is_empty());
    // AgeGroup depends only on Age
    assert_eq!(AgeGroup::non_derived_dependencies(), [Age::id()]);

    // Age has several dependents. This assert may break if you add or remove the properties in this test module.
    let mut expected_dependents = [
        AgeGroup::id(),
        DerivedProp::id(),
        ProfileNAW::id(),
        ProfileAWN::id(),
        ProfileWAN::id(),
    ];
    expected_dependents.sort_unstable();
    assert_eq!(Age::dependents(), expected_dependents);
}

#[test]
fn test_get_display() {
    let mut context = Context::new();
    let person = context.add_entity((POu32(Some(42)), Pu32(22))).unwrap();
    assert_eq!(
        format!(
            "{:}",
            POu32::get_display(&context.get_property::<_, POu32>(person))
        ),
        "42"
    );
    assert_eq!(
        format!(
            "{:}",
            Pu32::get_display(&context.get_property::<_, Pu32>(person))
        ),
        "Pu32(22)"
    );
    let person2 = context.add_entity((POu32(None), Pu32(11))).unwrap();
    assert_eq!(
        format!(
            "{:}",
            POu32::get_display(&context.get_property::<_, POu32>(person2))
        ),
        "None"
    );
}

#[test]
fn test_debug_trait() {
    let property = Pu32(11);
    let debug_str = format!("{:?}", property);
    assert_eq!(debug_str, "Pu32(11)");

    let property = POu32(Some(22));
    let debug_str = format!("{:?}", property);
    assert_eq!(debug_str, "POu32(Some(22))");
}
