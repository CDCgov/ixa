use std::fmt;

use ixa::impl_derived_property;
use ixa::prelude::*;
use rand_distr::{Exp, Uniform};

use crate::parameters_loader::Parameters;

define_rng!(PeopleRng);

static MAX_AGE: u8 = 100;

define_entity!(Person);

define_property!(
    enum InfectionStatus {
        S,
        I,
        R,
    },
    Person,
    default_const = InfectionStatus::S
);
define_property!(
    struct Age(pub u8),
    Person
);
define_property!(
    struct Alive(pub bool),
    Person,
    default_const = Alive(true)
);

// We declare the type ourselves so we can derive `Hash`.
#[derive(
    Debug, PartialEq, Eq, Clone, Copy, ixa::serde::Serialize, ixa::serde::Deserialize, Hash,
)]
pub enum AgeGroupRisk {
    NewBorn,
    General,
    OldAdult,
}

impl_derived_property!(AgeGroupRisk, Person, [Age], [], |age| {
    if age.0 <= 1 {
        AgeGroupRisk::NewBorn
    } else if age.0 <= 65 {
        AgeGroupRisk::General
    } else {
        AgeGroupRisk::OldAdult
    }
});

impl fmt::Display for AgeGroupRisk {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}

fn schedule_aging(context: &mut Context, person_id: PersonId) {
    let is_alive: Alive = context.get_property(person_id);
    if is_alive.0 {
        let prev_age: Age = context.get_property(person_id);
        context.set_property(person_id, Age(prev_age.0 + 1));
        let next_age_event = context.get_current_time() + 365.0;
        context.add_plan(next_age_event, move |context| {
            schedule_aging(context, person_id);
        });
    }
}

fn schedule_birth(context: &mut Context) {
    let parameters = context
        .get_global_property_value(Parameters)
        .unwrap()
        .clone();
    let person = context.add_entity((Age(0),)).unwrap();
    context.add_plan(context.get_current_time() + 365.0, move |context| {
        schedule_aging(context, person);
    });

    let next_birth_event = context.get_current_time()
        + context.sample_distr(PeopleRng, Exp::new(parameters.birth_rate).unwrap());
    context.add_plan(next_birth_event, move |context| {
        schedule_birth(context);
    });
}

fn schedule_death(context: &mut Context) {
    let parameters = context
        .get_global_property_value(Parameters)
        .unwrap()
        .clone();

    if let Some(person) = context.sample_entity(PeopleRng, (Alive(true),)) {
        context.set_property(person, Alive(false));

        let next_death_event = context.get_current_time()
            + context.sample_distr(PeopleRng, Exp::new(parameters.death_rate).unwrap());

        context.add_plan(next_death_event, |context| {
            schedule_death(context);
        });
    }
}

pub fn init(context: &mut Context) {
    let parameters = context
        .get_global_property_value(Parameters)
        .unwrap()
        .clone();

    for _ in 0..parameters.population {
        let age: u8 = context.sample_range(PeopleRng, 0..MAX_AGE);
        let person = context.add_entity((Age(age),)).unwrap();
        let birthday = context.sample_distr(PeopleRng, Uniform::new(0.0, 365.0).unwrap());
        context.add_plan(365.0 + birthday, move |context| {
            schedule_aging(context, person);
        });
    }

    // Plan for births and deaths
    if parameters.birth_rate > 0.0 {
        context.add_plan(0.0, |context| {
            schedule_birth(context);
        });
    }
    if parameters.death_rate > 0.0 {
        context.add_plan(0.0, |context| {
            schedule_death(context);
        });
    }
}

#[cfg(test)]
mod test {
    // Silence spurious unused import warnings.
    #![allow(unused_imports)]
    use std::cell::RefCell;
    use std::rc::Rc;

    use ixa::context::Context;

    use super::*;
    use crate::parameters_loader::{FoiAgeGroups, ParametersValues};

    #[test]
    fn test_birth_death() {
        let mut context = Context::new();

        let person1 = context.add_entity((Age(10),)).unwrap();
        let person2 = Rc::<RefCell<Option<PersonId>>>::new(RefCell::new(None));
        let person2_clone = Rc::clone(&person2);

        context.add_plan(380.0, move |context| {
            *person2_clone.borrow_mut() = Some(context.add_entity((Age(0),)).unwrap());
        });
        context.add_plan(400.0, move |context| {
            context.set_property(person1, Alive(false));
        });
        context.add_plan(390.0, |context| {
            let pop = context.query_entity_count((Alive(true),));
            assert_eq!(pop, 2);
        });
        context.add_plan(401.0, |context| {
            let pop = context.query_entity_count((Alive(true),));
            assert_eq!(pop, 1);
        });
        context.execute();
        let population = context.get_entity_count::<Person>();

        // Even if these people have died during simulation, we can still get their properties
        let age_0: Age = context.get_property(person1);
        let age_1: Age = context.get_property((*person2).borrow().unwrap());
        assert_eq!(age_0.0, 10);
        assert_eq!(age_1.0, 0);

        // Ixa population contains all individuals ever created
        assert_eq!(population, 2);
    }

    #[test]
    #[should_panic]
    fn test_null_birth_rates() {
        let p_values = ParametersValues {
            population: 10,
            max_time: 10.0,
            seed: 42,
            birth_rate: 0.0,
            death_rate: 0.1,
            foi_groups: Vec::<FoiAgeGroups>::new(),
            infection_duration: 5.0,
            output_file: ".".to_string(),
            demographic_output_file: ".".to_string(),
        };

        let mut context = Context::new();
        context
            .set_global_property_value(Parameters, p_values.clone())
            .unwrap();
        context.init_random(p_values.seed);

        schedule_birth(&mut context);
    }

    #[test]
    #[should_panic]
    fn test_null_death_rates() {
        let p_values = ParametersValues {
            population: 10,
            max_time: 10.0,
            seed: 42,
            birth_rate: 0.1,
            death_rate: 0.0,
            foi_groups: Vec::<FoiAgeGroups>::new(),
            infection_duration: 5.0,
            output_file: ".".to_string(),
            demographic_output_file: ".".to_string(),
        };

        let mut context = Context::new();
        context
            .set_global_property_value(Parameters, p_values.clone())
            .unwrap();
        context.init_random(p_values.seed);
        let _person = context.add_entity((Age(0),)).unwrap();
        schedule_death(&mut context);
    }

    #[test]
    fn test_sample_person_group() {
        let mut context = Context::new();
        let age_vec = vec![0, 5, 62, 80];
        let years = 5.0;
        let age_groups = [
            AgeGroupRisk::NewBorn,
            AgeGroupRisk::General,
            AgeGroupRisk::General,
            AgeGroupRisk::OldAdult,
        ];
        let mut people = Vec::<PersonId>::new();
        for age in &age_vec {
            people.push(context.add_entity((Age(*age),)).unwrap());
        }

        for i in 0..people.len() {
            let person = people[i];
            context.add_plan(365.0, move |context| {
                schedule_aging(context, person);
            });
            let age_group = age_groups[i];
            assert_eq!(
                age_group,
                context.get_property::<Person, AgeGroupRisk>(people[i])
            );
        }

        // Plan to check in 5 years
        let future_age_groups = [
            AgeGroupRisk::General,
            AgeGroupRisk::General,
            AgeGroupRisk::OldAdult,
            AgeGroupRisk::OldAdult,
        ];
        context.add_plan(years * 365.0, move |context| {
            for i in 0..people.len() {
                let age_group = future_age_groups[i];
                assert_eq!(
                    age_group,
                    context.get_property::<Person, AgeGroupRisk>(people[i])
                );
            }
        });

        context.add_plan((years * 365.0) + 1.0, |context| {
            context.shutdown();
        });
        context.execute();
    }
}
