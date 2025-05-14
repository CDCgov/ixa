use crate::parameters_loader::Parameters;
use ixa::people::PersonId;
use ixa::prelude::*;

use rand_distr::{Exp, Uniform};
use serde::{Deserialize, Serialize};
use std::fmt;

define_rng!(PeopleRng);

static MAX_AGE: u8 = 100;

#[derive(Debug, Hash, Eq, PartialEq, Clone, Copy, Serialize, Deserialize)]
pub enum InfectionStatusValue {
    S,
    I,
    R,
}

#[derive(Deserialize, Serialize, Copy, Clone, PartialEq, Eq, Debug, Hash)]
pub enum AgeGroupRisk {
    NewBorn,
    General,
    OldAdult,
}

impl fmt::Display for AgeGroupRisk {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}

define_person_property_with_default!(
    InfectionStatus,
    InfectionStatusValue,
    InfectionStatusValue::S
);
define_person_property!(Age, u8);
define_person_property_with_default!(Alive, bool, true);
define_derived_property!(AgeGroupFoi, AgeGroupRisk, [Age], |age| {
    if age <= 1 {
        AgeGroupRisk::NewBorn
    } else if age <= 65 {
        AgeGroupRisk::General
    } else {
        AgeGroupRisk::OldAdult
    }
});

fn schedule_aging(context: &mut Context, person_id: PersonId) {
    if context.get_person_property(person_id, Alive) {
        let prev_age = context.get_person_property(person_id, Age);
        context.set_person_property(person_id, Age, prev_age + 1);
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
    let person = context.add_person((Age, 0)).unwrap();
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

    if let Some(person) = context.sample_person(PeopleRng, (Alive, true)) {
        context.set_person_property(person, Alive, false);

        let next_death_event = context.get_current_time()
            + context.sample_distr(PeopleRng, Exp::new(parameters.death_rate).unwrap());

        context.add_plan(next_death_event, move |context| {
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
        let person = context.add_person((Age, age)).unwrap();
        let birthday = context.sample_distr(PeopleRng, Uniform::new(0.0, 365.0));
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
    use super::*;
    use crate::parameters_loader::{FoiAgeGroups, ParametersValues};
    use ixa::context::Context;
    use std::cell::RefCell;
    use std::rc::Rc;

    #[test]
    fn test_birth_death() {
        let mut context = Context::new();

        let person1 = context.add_person((Age, 10)).unwrap();
        let person2 = Rc::<RefCell<Option<PersonId>>>::new(RefCell::new(None));
        let person2_clone = Rc::clone(&person2);

        context.add_plan(380.0, move |context| {
            *person2_clone.borrow_mut() = Some(context.add_person((Age, 0)).unwrap());
        });
        context.add_plan(400.0, move |context| {
            context.set_person_property(person1, Alive, false);
        });
        context.add_plan(390.0, |context| {
            let pop = context.query_people_count((Alive, true));
            assert_eq!(pop, 2);
        });
        context.add_plan(401.0, |context| {
            let pop = context.query_people_count((Alive, true));
            assert_eq!(pop, 1);
        });
        context.execute();
        let population = context.get_current_population();

        // Even if these people have died during simulation, we can still get their properties
        let age_0 = context.get_person_property(person1, Age);
        let age_1 = context.get_person_property((*person2).borrow().unwrap(), Age);
        assert_eq!(age_0, 10);
        assert_eq!(age_1, 0);

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
        let _person = context.add_person((Age, 0)).unwrap();
        schedule_death(&mut context);
    }

    #[test]
    fn test_sample_person_group() {
        let mut context = Context::new();
        let age_vec = vec![0, 5, 62, 80];
        let years = 5.0;
        let age_groups = vec![
            AgeGroupRisk::NewBorn,
            AgeGroupRisk::General,
            AgeGroupRisk::General,
            AgeGroupRisk::OldAdult,
        ];
        let mut people = Vec::<PersonId>::new();
        for age in &age_vec {
            people.push(context.add_person((Age, *age)).unwrap());
        }

        for i in 0..people.len() {
            let person = people[i];
            context.add_plan(365.0, move |context| {
                schedule_aging(context, person);
            });
            let age_group = age_groups[i];
            assert_eq!(
                age_group,
                context.get_person_property(people[i], AgeGroupFoi)
            );
        }

        // Plan to check in 5 years
        let future_age_groups = vec![
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
                    context.get_person_property(people[i], AgeGroupFoi)
                );
            }
        });

        context.add_plan((years * 365.0) + 1.0, |context| {
            context.shutdown();
        });
        context.execute();
    }
}
