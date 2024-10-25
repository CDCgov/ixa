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
        println!("Person {person_id:?} infected, scheduling recovery");
        let plan_id = context
            .add_plan(recovery_time, move |context| {
                context.set_person_property(person_id, InfectionStatusType, InfectionStatus::R);
            })
            .clone();
        let plans_data_container = context.get_data_container_mut(InfectionPlansPlugin);
        plans_data_container
            .plans_map
            .entry(person_id)
            .or_default()
            .insert(plan_id.clone());

        println!("Person {:?} - plan Id: {:?}", person_id, plan_id.clone());
    }
}

fn remove_recovery_plan_data(context: &mut Context, person_id: PersonId) {
    let plans_data_container = context.get_data_container_mut(InfectionPlansPlugin);
    plans_data_container.plans_map.remove(&person_id);
    println!("Person {person_id:?} - plans removed");
}

fn cancel_recovery_plans(context: &mut Context, person_id: PersonId) {
    println!("Attempting to cancel plans for Person {person_id:?}");
    let plans_data_container = context.get_data_container_mut(InfectionPlansPlugin);
    let plans_set = plans_data_container
        .plans_map
        .get(&person_id)
        .unwrap_or(&HashSet::<plan::Id>::new())
        .clone();

    for plan_id in plans_set {
        println!("Canceling plan {:?}", plan_id.clone());
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
        println!("Person {:?} has recovered", event.person_id);
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
