//! Ixa is a dependency under another name here, so `::ixa` does not resolve.

use sim::prelude::*;

define_entity!(Person);
define_property!(struct Age(u8), Person, default_const = Age(0));
define_property!(struct Weight(f64), Person, default_const = Weight(0.0));
define_derived_property!(
    struct HalfWeight(f64),
    Person,
    [Weight],
    |weight| HalfWeight(weight.0 / 2.0)
);

#[derive(Debug, Clone, Copy, EqHash, serde::Serialize, serde::Deserialize)]
#[eq_hash(crate = sim)]
pub struct Height(f64);
impl_property!(Height, Person, default_const = Height(0.0));

#[test]
fn properties_work_under_a_renamed_crate() {
    let mut context = Context::new();
    context.index_property::<Person, Weight>();
    let person = context
        .add_entity(with!(Person, Age(3), Weight(-0.0), Height(1.5)))
        .unwrap();
    assert_eq!(context.query_entity_count(with!(Person, Weight(0.0))), 1);
    assert_eq!(
        context.get_property::<Person, HalfWeight>(person),
        HalfWeight(0.0)
    );
    assert_eq!(context.get_property::<Person, Height>(person), Height(1.5));
}
