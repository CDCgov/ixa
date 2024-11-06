use crate::population_manager::{Age, AgeGroupFoi, AgeGroupRisk, Alive};
use crate::Parameters;
use ixa::{
    context::Context,
    create_report_trait,
    global_properties::ContextGlobalPropertiesExt,
    people::{ContextPeopleExt, PersonCreatedEvent, PersonPropertyChangeEvent},
    report::{ContextReportExt, Report},
};
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::path::PathBuf;

#[derive(Serialize, Deserialize, Clone)]
struct PersonReportItem {
    time: f64,
    person_id: String,
    age_group: AgeGroupRisk,
    property: String,
    property_prev: String,
    property_current: String,
}

create_report_trait!(PersonReportItem);

fn handle_person_created(context: &mut Context, event: PersonCreatedEvent) {
    let person = event.person_id;
    let age_group_person = context.get_person_property(person, AgeGroupFoi);
    context.send_report(PersonReportItem {
        time: context.get_current_time(),
        person_id: format!("{person}"),
        age_group: age_group_person,
        property: "Created".to_string(),
        property_prev: String::new(),
        property_current: String::new(),
    });
}

fn handle_person_aging(context: &mut Context, event: PersonPropertyChangeEvent<Age>) {
    let person = event.person_id;
    let age_group_person = context.get_person_property(person, AgeGroupFoi);
    context.send_report(PersonReportItem {
        time: context.get_current_time(),
        person_id: format!("{person}"),
        age_group: age_group_person,
        property: "Age".to_string(),
        property_prev: format!("{:?}", event.previous),
        property_current: format!("{:?}", event.current),
    });
}

fn handle_death_events(context: &mut Context, event: PersonPropertyChangeEvent<Alive>) {
    if !event.current {
        let person = event.person_id;
        let age_group_person = context.get_person_property(person, AgeGroupFoi);
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

pub fn init(context: &mut Context) {
    let parameters = context
        .get_global_property_value(Parameters)
        .unwrap()
        .clone();

    let current_dir = Path::new(file!()).parent().unwrap();
    context
        .report_options()
        .directory(PathBuf::from(current_dir));

    context.add_report::<PersonReportItem>(&parameters.demographic_output_file);
    context.subscribe_to_event(|context, event: PersonCreatedEvent| {
        handle_person_created(context, event);
    });
    context.subscribe_to_event(|context, event: PersonPropertyChangeEvent<Alive>| {
        handle_death_events(context, event);
    });

    context.subscribe_to_event(|context, event: PersonPropertyChangeEvent<Age>| {
        handle_person_aging(context, event);
    });
}
