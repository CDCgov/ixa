use ixa::prelude::*;

use crate::parameters_loader::Parameters;

define_entity!(Person);

define_property!(
    enum DiseaseStatus {
        S,
        I,
        R,
    },
    Person,
    default_const = DiseaseStatus::S
);

define_property!(
    struct InfectionTime(pub Option<f64>),
    Person,
    default_const = InfectionTime(None)
);

pub fn init(context: &mut Context) {
    let parameters = context
        .get_global_property_value(Parameters)
        .unwrap()
        .clone();
    for _ in 0..parameters.population {
        let _: PersonId = context.add_entity(()).unwrap();
    }
}
