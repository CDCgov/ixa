use ixa::context::Context;
use ixa::people::ContextPeopleExt;
use ixa::{define_person_property, define_person_property_with_default};
use serde::{Deserialize, Serialize};

use crate::POPULATION;

#[derive(Deserialize, Serialize, Copy, Clone, PartialEq, Eq, Debug)]
pub enum DiseaseStatus {
    S,
    I,
    R,
}

define_person_property_with_default!(DiseaseStatusType, DiseaseStatus, DiseaseStatus::S);

pub fn init(context: &mut Context) {
    for _ in 0..POPULATION {
        context.add_person();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::POPULATION;
    use crate::SEED;
    use ixa::context::Context;
    use ixa::people::ContextPeopleExt;
    use ixa::random::ContextRandomExt;

    #[test]
    fn test_person_creation_default_properties() {
        let mut context = Context::new();
        context.init_random(SEED);
        init(&mut context);

        let population_size = context.get_current_population();
        assert_eq!(population_size as u64, POPULATION);
        for i in 0..population_size {
            let status = context.get_person_property(context.get_person_id(i), DiseaseStatusType);
            assert_eq!(status, DiseaseStatus::S);
        }
    }
}
