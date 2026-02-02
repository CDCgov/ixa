use std::path::Path;

use ixa::entity::events::{EntityCreatedEvent, PropertyChangeEvent};
use ixa::prelude::*;
use serde::{Deserialize, Serialize};

use crate::population_manager::{Age, AgeGroupRisk, Alive, Person};
use crate::Parameters;

#[derive(Serialize, Deserialize, Clone)]
struct PersonReportItem {
    time: f64,
    person_id: String,
    age_group: AgeGroupRisk,
    property: String,
    property_prev: String,
    property_current: String,
}

define_report!(PersonReportItem);

fn handle_person_created(context: &mut Context, event: EntityCreatedEvent<Person>) {
    let person = event.entity_id;
    let age_group_person: AgeGroupRisk = context.get_property(person);
    context.send_report(PersonReportItem {
        time: context.get_current_time(),
        person_id: format!("{person}"),
        age_group: age_group_person,
        property: "Created".to_string(),
        property_prev: String::new(),
        property_current: String::new(),
    });
}

fn handle_person_aging(context: &mut Context, event: PropertyChangeEvent<Person, Age>) {
    let person = event.entity_id;
    let age_group_person: AgeGroupRisk = context.get_property(person);
    context.send_report(PersonReportItem {
        time: context.get_current_time(),
        person_id: format!("{person}"),
        age_group: age_group_person,
        property: "Age".to_string(),
        property_prev: format!("{:?}", event.previous),
        property_current: format!("{:?}", event.current),
    });
}

fn handle_death_events(context: &mut Context, event: PropertyChangeEvent<Person, Alive>) {
    if !event.current.0 {
        let person = event.entity_id;
        let age_group_person: AgeGroupRisk = context.get_property(person);
        context.send_report(PersonReportItem {
            time: context.get_current_time(),
            person_id: format!("{person}"),
            age_group: age_group_person,
            property: "Alive".to_string(),
            property_prev: format!("{:?}", event.previous),
            property_current: format!("{:?}", event.current),
        });
    }
}

pub fn init(context: &mut Context, output_path: &Path) -> Result<(), IxaError> {
    let parameters = context
        .get_global_property_value(Parameters)
        .unwrap()
        .clone();

    let current_dir = output_path.to_path_buf();
    context
        .report_options()
        .directory(current_dir)
        .overwrite(true); // Not recommended for production. See `basic-infection/incidence-report`.

    context.add_report::<PersonReportItem>(&parameters.demographic_output_file)?;
    context.subscribe_to_event(|context, event: EntityCreatedEvent<Person>| {
        handle_person_created(context, event);
    });
    context.subscribe_to_event(|context, event: PropertyChangeEvent<Person, Alive>| {
        handle_death_events(context, event);
    });
    context.subscribe_to_event(|context, event: PropertyChangeEvent<Person, Age>| {
        handle_person_aging(context, event);
    });

    Ok(())
}
