use ixa::context::Context;
use ixa::global_properties::ContextGlobalPropertiesExt;
use ixa::people::ContextPeopleExt;
use ixa::{define_person_property, define_person_property_with_default};
use serde::{Deserialize, Serialize};
use strum_macros::EnumIter;

use crate::parameters_loader::Parameters;

#[derive(Deserialize, Serialize, Copy, Clone, PartialEq, Eq, Debug, EnumIter)]
pub enum DiseaseStatus {
    S,
    I,
    R,
}

define_person_property_with_default!(DiseaseStatusType, DiseaseStatus, DiseaseStatus::S);
define_person_property_with_default!(InfectionTime, Option<f64>, None);

pub fn init(context: &mut Context) {
    let parameters = context.get_global_property_value(Parameters).clone();
    for _ in 0..parameters.population {
        context.add_person();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use ixa::context::Context;
    use ixa::global_properties::ContextGlobalPropertiesExt;
    use ixa::people::ContextPeopleExt;
    use ixa::random::ContextRandomExt;

    use crate::parameters_loader::ParametersValues;

    #[test]
    fn test_person_creation_default_properties() {
        let p_values = ParametersValues {
            population: 1,
            max_time: 10.0,
            seed: 42,
            foi: 0.15,
            foi_sin_shift: 3.0,
            infection_duration: 5.0,
            plan_period: 1.0,
            output_dir: ".".to_string(),
            output_file: ".".to_string(),
        };
        let mut context = Context::new();
        context.set_global_property_value(Parameters, p_values);
        let parameters = context.get_global_property_value(Parameters).clone();
        context.init_random(parameters.seed);
        init(&mut context);

        let population_size = context.get_current_population();
        assert_eq!(population_size, parameters.population);
        for i in 0..population_size {
            let status = context.get_person_property(context.get_person_id(i), DiseaseStatusType);
            let time = context.get_person_property(context.get_person_id(i), InfectionTime);
            assert_eq!(status, DiseaseStatus::S);
            assert!(time.is_none());
        }
    }
}
