use ixa::entity::events::PropertyChangeEvent;
use ixa::plan::PlanId;
use ixa::prelude::*;
use ixa::{HashMap, HashMapExt, HashSet, HashSetExt};
use rand_distr::Exp;

use crate::population_manager::{Alive, InfectionStatus, Person, PersonId};
use crate::Parameters;

define_rng!(InfectionRng1);
define_data_plugin!(
    InfectionPlansPlugin,
    InfectionPlansData,
    InfectionPlansData {
        plans_map: HashMap::<PersonId, HashSet<PlanId>>::new(),
    }
);

#[derive(Debug)]
struct InfectionPlansData {
    plans_map: HashMap<PersonId, HashSet<PlanId>>,
}

fn schedule_recovery(context: &mut Context, person_id: PersonId) {
    let parameters = context
        .get_global_property_value(Parameters)
        .unwrap()
        .clone();
    let infection_duration = parameters.infection_duration;
    let recovery_time = context.get_current_time()
        + context.sample_distr(InfectionRng1, Exp::new(1.0 / infection_duration).unwrap());
    let is_alive: Alive = context.get_property(person_id);

    if is_alive.0 {
        let plan_id = context.add_plan(recovery_time, move |context| {
            context.set_property(person_id, InfectionStatus::R);
        });
        let plans_data_container = context.get_data_mut(InfectionPlansPlugin);
        plans_data_container
            .plans_map
            .entry(person_id)
            .or_default()
            .insert(plan_id);
    }
}

fn remove_recovery_plan_data(context: &mut Context, person_id: PersonId) {
    let plans_data_container = context.get_data_mut(InfectionPlansPlugin);
    plans_data_container.plans_map.remove(&person_id);
}

fn cancel_recovery_plans(context: &mut Context, person_id: PersonId) {
    let plans_data_container = context.get_data_mut(InfectionPlansPlugin);
    let plans_set = plans_data_container
        .plans_map
        .get(&person_id)
        .unwrap_or(&HashSet::<PlanId>::new())
        .clone();

    for plan_id in plans_set {
        context.cancel_plan(&plan_id);
    }

    remove_recovery_plan_data(context, person_id);
}

fn handle_infection_status_change(
    context: &mut Context,
    event: PropertyChangeEvent<Person, InfectionStatus>,
) {
    match event.current {
        InfectionStatus::I => {
            schedule_recovery(context, event.entity_id);
        }
        InfectionStatus::R => {
            remove_recovery_plan_data(context, event.entity_id);
        }
        _ => {}
    }
}

fn handle_person_removal(context: &mut Context, event: PropertyChangeEvent<Person, Alive>) {
    if !event.current.0 {
        cancel_recovery_plans(context, event.entity_id);
    }
}
pub fn init(context: &mut Context) {
    context.subscribe_to_event(
        move |context, event: PropertyChangeEvent<Person, InfectionStatus>| {
            handle_infection_status_change(context, event);
        },
    );

    context.subscribe_to_event(move |context, event: PropertyChangeEvent<Person, Alive>| {
        handle_person_removal(context, event);
    });
}

#[cfg(test)]
mod test {
    // Silence spurious unused import warnings.
    #![allow(unused_imports)]
    use ixa::prelude::*;

    use super::*;
    use crate::parameters_loader::{FoiAgeGroups, ParametersValues};
    use crate::population_manager::Age;

    define_data_plugin!(RecoveryPlugin, usize, 0);
    define_data_plugin!(PlansPlugin, usize, 0);

    #[test]
    fn test_handle_infection_change_with_deaths() {
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
        init(&mut context);

        context.subscribe_to_event(
            move |context, event: PropertyChangeEvent<Person, InfectionStatus>| {
                if event.current == InfectionStatus::R {
                    *context.get_data_mut(RecoveryPlugin) += 1;
                }
            },
        );

        let population_size: usize = 10;
        for index in 0..population_size {
            let person = context.add_entity(all!(Person, Age(0))).unwrap();

            context.add_plan(1.0, move |context| {
                context.set_property(person, InfectionStatus::I);
            });

            if index == 0 {
                context.add_plan(1.1, move |context| {
                    context.set_property(person, Alive(false));
                });
            }
        }

        context.execute();
        assert_eq!(population_size, context.get_entity_count::<Person>());
        let recovered_size: usize = *context.get_data(RecoveryPlugin);

        assert_eq!(recovered_size, population_size - 1);
    }

    #[test]
    fn test_cancel_null_plan() {
        let mut context = Context::new();

        context.init_random(42);
        init(&mut context);

        let person = context.add_entity(all!(Person, Age(0))).unwrap();
        context.add_plan(1.1, move |context| {
            cancel_recovery_plans(context, person);
        });

        context.add_plan(1.2, move |context| {
            cancel_recovery_plans(context, person);
        });

        context.execute();
    }
}
