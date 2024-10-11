use ixa::{
    context::Context, create_report_trait, global_properties::ContextGlobalPropertiesExt, people::{ContextPeopleExt, PersonCreatedEvent, PersonRemovedEvent}, report::{ContextReportExt, Report}
};
use std::path::PathBuf;
use serde::{Deserialize, Serialize};

use crate::{
    population_manager::Age,
    Parameters,
};

#[derive(Serialize, Deserialize, Clone)]
struct PersonReportItem {
    time: f64,
    person_id: String,
    age: u8,
    event: String,
}

create_report_trait!(PersonReportItem);

fn handle_person_created(
    context: &mut Context,
    event: PersonCreatedEvent,
) {
    let person = event.person_id;
    context.send_report(PersonReportItem {
        time: context.get_current_time(),
        person_id: format!("{}", person),
        age: context.get_person_property(person, Age),
        event: "Created".to_string(),
    });
}

fn handle_person_removed(
    context: &mut Context,
    event: PersonRemovedEvent,
) {
    let person = event.person_id;
    context.send_report(PersonReportItem {
        time: context.get_current_time(),
        person_id: format!("{}", person),
        age: context.get_person_property(person, Age),
        event: "Removed".to_string(),
    });
}

pub fn init(context: &mut Context) {
    let parameters = context.get_global_property_value(Parameters).clone();
    context
        .report_options()
        .directory(PathBuf::from(parameters.output_dir));
    context.add_report::<PersonReportItem>(&parameters.output_people_file);
    context.subscribe_to_event(
        |context, event: PersonCreatedEvent| {            
            handle_person_created(context, event);
        },
    );

    context.subscribe_to_event(
        |context, event: PersonRemovedEvent| {            
            handle_person_removed(context, event);
        },
    );
}
