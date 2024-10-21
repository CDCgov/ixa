use ixa::{
    context::Context, create_report_trait, global_properties::ContextGlobalPropertiesExt, people::{ContextPeopleExt, PersonCreatedEvent}, report::{ContextReportExt, Report}
};
use std::path::PathBuf;
use std::path::Path;
use serde::{Deserialize, Serialize};
use crate::population_loader::{Age, AgeGroupType};
use crate::Parameters;

#[derive(Serialize, Deserialize, Clone)]
struct PersonReportItem {
    time: f64,
    person_id: String,
    age: u8,
    age_group: String,
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
        age_group: format!("{}", context.get_person_property(person, AgeGroupType)),
        event: "Created".to_string(),
    });
}

pub fn init(context: &mut Context) {
    let parameters = context.get_global_property_value(Parameters).clone();

    let current_dir = Path::new(file!()).parent().unwrap();
    context
        .report_options()
        .directory(PathBuf::from(current_dir));

    context.add_report::<PersonReportItem>(&parameters.demographic_output_file);
    context.subscribe_to_event(
        |context, event: PersonCreatedEvent| {            
            handle_person_created(context, event);
        },
    );
}
