use crate::population_manager::Alive;
use crate::population_manager::InfectionStatus;
use crate::population_manager::InfectionStatusType;
use crate::Parameters;
use ixa::context::Context;
use ixa::define_data_plugin;
use ixa::define_rng;
use ixa::global_properties::ContextGlobalPropertiesExt;
use ixa::people::{ContextPeopleExt, PersonId, PersonPropertyChangeEvent};
use ixa::plan;
use ixa::random::ContextRandomExt;
use rand_distr::Exp;
use std::collections::{HashMap, HashSet};

define_rng!(InfectionRng);
define_data_plugin!(
    InfectionPlansPlugin,
    InfectionPlansData,
    InfectionPlansData {
        plans_map: HashMap::<PersonId, HashSet::<plan::Id>>::new(),
    }
);

#[derive(Debug)]
struct InfectionPlansData {
    plans_map: HashMap<PersonId, HashSet<plan::Id>>,
}

fn schedule_recovery(context: &mut Context, person_id: PersonId) {
    let parameters = context.get_global_property_value(Parameters).clone();
    let infection_duration = parameters.infection_duration;
    let recovery_time = context.get_current_time()
        + context.sample_distr(InfectionRng, Exp::new(1.0 / infection_duration).unwrap());

    if context.get_person_property(person_id, Alive) {
        let plan_id = context.add_plan(recovery_time, move |context| {
            context.set_person_property(person_id, InfectionStatusType, InfectionStatus::R);
        });
        let plans_data_container = context.get_data_container_mut(InfectionPlansPlugin);
        plans_data_container
            .plans_map
            .entry(person_id)
            .or_default()
            .insert(plan_id);
    }
}

fn remove_recovery_plan_data(context: &mut Context, person_id: PersonId) {
    let plans_data_container = context.get_data_container_mut(InfectionPlansPlugin);
    plans_data_container.plans_map.remove(&person_id);
}

fn cancel_recovery_plans(context: &mut Context, person_id: PersonId) {
    let plans_data_container = context.get_data_container_mut(InfectionPlansPlugin);
    let plans_set = plans_data_container
        .plans_map
        .get(&person_id)
        .unwrap_or(&HashSet::<plan::Id>::new())
        .clone();

    for plan_id in plans_set {
        context.cancel_plan(&plan_id);
    }

    remove_recovery_plan_data(context, person_id);
}

fn handle_infection_status_change(
    context: &mut Context,
    event: PersonPropertyChangeEvent<InfectionStatusType>,
) {
    if matches!(event.current, InfectionStatus::I) {
        schedule_recovery(context, event.person_id);
    }
    if matches!(event.current, InfectionStatus::R) {
        remove_recovery_plan_data(context, event.person_id);
    }
}

fn handle_person_removal(context: &mut Context, event: PersonPropertyChangeEvent<Alive>) {
    if !event.current {
        cancel_recovery_plans(context, event.person_id);
    }
}
pub fn init(context: &mut Context) {
    context.subscribe_to_event(
        move |context, event: PersonPropertyChangeEvent<InfectionStatusType>| {
            handle_infection_status_change(context, event);
        },
    );

    context.subscribe_to_event(move |context, event: PersonPropertyChangeEvent<Alive>| {
        handle_person_removal(context, event);
    });
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::population_manager::ContextPopulationExt;
    use ixa::context::Context;
    use ixa::define_data_plugin;
    use ixa::global_properties::ContextGlobalPropertiesExt;
    use ixa::people::{ContextPeopleExt, PersonPropertyChangeEvent};
    use ixa::random::ContextRandomExt;

    use crate::parameters_loader::{FoiAgeGroups, ParametersValues};
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

        context.set_global_property_value(Parameters, p_values.clone());
        context.init_random(p_values.seed);
        init(&mut context);

        context.subscribe_to_event(
            move |context, event: PersonPropertyChangeEvent<InfectionStatusType>| {
                if matches!(event.current, InfectionStatus::R) {
                    *context.get_data_container_mut(RecoveryPlugin) += 1;
                }
            },
        );

        let population_size: usize = 10;
        for _ in 0..population_size {
            let person = context.create_new_person(0);

            context.add_plan(1.0, move |context| {
                context.set_person_property(person, InfectionStatusType, InfectionStatus::I);
            });
        }

        context.add_plan(1.1, move |context| {
            context.attempt_death(context.get_person_id(0));
        });

        context.execute();
        assert_eq!(population_size, context.get_current_population());
        let recovered_size: usize = *context.get_data_container(RecoveryPlugin).unwrap();

        assert_eq!(recovered_size, population_size - 1);
    }

    #[test]
    fn test_cancel_null_plan() {
        let mut context = Context::new();

        context.init_random(42);
        init(&mut context);

        let person = context.create_new_person(0);
        context.add_plan(1.1, move |context| {
            cancel_recovery_plans(context, person);
        });

        context.add_plan(1.2, move |context| {
            cancel_recovery_plans(context, person);
        });

        context.execute();
    }
}
