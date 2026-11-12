//! The `EqHash` derive: float-valued properties compare by value and can be indexed.

use ixa::prelude::*;
use ixa::HashMap;

define_entity!(Person);

define_property!(struct Weight(f64), Person, default_const = Weight(0.0));
define_property!(struct MaybeWeight(Option<f32>), Person, default_const = MaybeWeight(None));
define_property!(
    struct Dose {
        amount: f64,
        count: u8,
    },
    Person,
    default_const = Dose {
        amount: 0.0,
        count: 0
    }
);
define_property!(struct Position([f64; 2]), Person, default_const = Position([0.0, 0.0]));
define_derived_property!(
    struct HalfWeight(f64),
    Person,
    [Weight],
    |weight| HalfWeight(weight.0 / 2.0)
);

#[derive(Debug, Clone, Copy, EqHash, serde::Serialize, serde::Deserialize)]
pub enum Treatment {
    Untreated,
    Antiviral { dose_mg: f64 },
    Vaccinated(u8, f32),
}
impl_property!(Treatment, Person, default_const = Treatment::Untreated);

// A nested property type goes through its own `Eq + Hash`, which `EqHash` provided.
#[derive(Debug, Clone, Copy, EqHash, serde::Serialize, serde::Deserialize)]
pub struct Exposure {
    dose: Dose,
    weight: Weight,
}
impl_property!(
    Exposure,
    Person,
    default_const = Exposure {
        dose: Dose {
            amount: 0.0,
            count: 0
        },
        weight: Weight(0.0)
    }
);

// The crate path can be given explicitly, as the property macros do with `$crate`.
mod reexport {
    pub use ixa as sim;
}
#[derive(Debug, Clone, Copy, EqHash)]
#[eq_hash(crate = reexport::sim)]
pub struct ExplicitCratePath(f64);

fn hash_of<T: std::hash::Hash>(value: &T) -> u64 {
    use std::hash::Hasher;
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    value.hash(&mut hasher);
    hasher.finish()
}

#[test]
fn floats_compare_by_value_with_nan_reflexive() {
    assert_eq!(Weight(1.5), Weight(1.5));
    assert_ne!(Weight(1.5), Weight(2.5));
    assert_eq!(Weight(0.0), Weight(-0.0));
    assert_eq!(hash_of(&Weight(0.0)), hash_of(&Weight(-0.0)));
    assert_eq!(Weight(f64::NAN), Weight(f64::NAN));
    assert_eq!(hash_of(&Weight(f64::NAN)), hash_of(&Weight(-f64::NAN)));
    assert_ne!(Weight(f64::INFINITY), Weight(f64::NEG_INFINITY));

    assert_eq!(MaybeWeight(Some(-0.0)), MaybeWeight(Some(0.0)));
    assert_ne!(MaybeWeight(None), MaybeWeight(Some(0.0)));
    assert_eq!(Position([0.0, f64::NAN]), Position([-0.0, f64::NAN]));
}

#[test]
fn mixed_fields_compare_every_field() {
    let dose = Dose {
        amount: 1.0,
        count: 2,
    };
    assert_eq!(
        dose,
        Dose {
            amount: 1.0,
            count: 2
        }
    );
    assert_ne!(
        dose,
        Dose {
            amount: 1.0,
            count: 3
        }
    );
    assert_ne!(
        dose,
        Dose {
            amount: 2.0,
            count: 2
        }
    );
    let exposure = Exposure {
        dose,
        weight: Weight(-0.0),
    };
    assert_eq!(
        exposure,
        Exposure {
            dose,
            weight: Weight(0.0)
        }
    );
    assert_eq!(
        hash_of(&exposure),
        hash_of(&Exposure {
            dose,
            weight: Weight(0.0)
        })
    );
}

// A constant named like a plain binding would turn the derive's patterns into const patterns.
mod binding_names {
    #[allow(non_upper_case_globals, dead_code)]
    const left_0: u8 = 1;
    #[allow(non_upper_case_globals, dead_code)]
    const right_0: u8 = 1;

    #[derive(Debug, Clone, Copy, ixa::EqHash)]
    pub struct Shadowed(pub u8);
}

#[test]
fn bindings_do_not_collide_with_constants_in_scope() {
    assert_eq!(binding_names::Shadowed(2), binding_names::Shadowed(2));
    assert_ne!(binding_names::Shadowed(2), binding_names::Shadowed(3));
}

#[test]
fn explicit_crate_path_is_used() {
    assert_eq!(ExplicitCratePath(-0.0), ExplicitCratePath(0.0));
}

define_property!(
    enum Stage {
        Early,
        Late,
    },
    Person,
    default_const = Stage::Early
);

// A `define_property!` enum keeps the std derives, so a constant of it can be a pattern.
#[test]
fn macro_declared_enum_constants_are_patterns() {
    const DEFAULT_STAGE: Stage = Stage::Early;
    assert!(matches!(Stage::Early, DEFAULT_STAGE));
    assert!(!matches!(Stage::Late, DEFAULT_STAGE));
}

#[test]
fn enums_compare_variant_then_fields() {
    assert_eq!(Treatment::Untreated, Treatment::Untreated);
    assert_ne!(Treatment::Untreated, Treatment::Antiviral { dose_mg: 0.0 });
    assert_eq!(
        Treatment::Antiviral { dose_mg: f64::NAN },
        Treatment::Antiviral { dose_mg: f64::NAN }
    );
    assert_ne!(
        Treatment::Antiviral { dose_mg: 1.0 },
        Treatment::Antiviral { dose_mg: 2.0 }
    );
    assert_eq!(
        Treatment::Vaccinated(1, 0.0),
        Treatment::Vaccinated(1, -0.0)
    );
    assert_ne!(Treatment::Vaccinated(1, 0.0), Treatment::Vaccinated(2, 0.0));
    assert_ne!(
        hash_of(&Treatment::Vaccinated(1, 0.0)),
        hash_of(&Treatment::Vaccinated(2, 0.0))
    );
}

#[test]
fn float_properties_index_and_query() {
    let mut context = Context::new();
    context.index_property::<Person, Weight>();
    context.index_property::<Person, Treatment>();
    for weight in [1.0, 2.0, 2.0, -0.0, f64::NAN] {
        context
            .add_entity(with!(
                Person,
                Weight(weight),
                Treatment::Antiviral { dose_mg: weight }
            ))
            .unwrap();
    }

    assert_eq!(context.query_entity_count(with!(Person, Weight(2.0))), 2);
    assert_eq!(context.query_entity_count(with!(Person, Weight(0.0))), 1);
    assert_eq!(
        context.query_entity_count(with!(Person, Weight(f64::NAN))),
        1
    );
    assert_eq!(context.query_entity_count(with!(Person, Weight(3.0))), 0);
    assert_eq!(
        context.query_entity_count(with!(Person, Treatment::Antiviral { dose_mg: 2.0 })),
        2
    );
    assert_eq!(
        context.query_entity_count(with!(Person, Treatment::Untreated)),
        0
    );

    // Derived float properties hash too.
    let person = context.add_entity(with!(Person, Weight(5.0))).unwrap();
    let mut halves: HashMap<HalfWeight, usize> = HashMap::default();
    *halves
        .entry(context.get_property::<Person, HalfWeight>(person))
        .or_default() += 1;
    assert_eq!(halves.get(&HalfWeight(2.5)), Some(&1));
}
