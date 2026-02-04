use std::path::Path;

use ixa::entity::events::PropertyChangeEvent;
use ixa::prelude::*;
use serde::Serialize;

use crate::population_manager::{Age, AgeGroupRisk, InfectionStatus, Person};
use crate::Parameters;

#[derive(Serialize, Clone)]
struct IncidenceReportItem {
    time: f64,
    person_id: String,
    age_group: AgeGroupRisk,
    age: u8,
    infection_status: InfectionStatus,
}

define_report!(IncidenceReportItem);

fn handle_infection_status_change(
    context: &mut Context,
    event: PropertyChangeEvent<Person, InfectionStatus>,
) {
    let age_person: Age = context.get_property(event.entity_id);
    let age_group_person: AgeGroupRisk = context.get_property(event.entity_id);
    context.send_report(IncidenceReportItem {
        time: context.get_current_time(),
        person_id: format!("{}", event.entity_id),
        age_group: age_group_person,
        age: age_person.0,
        infection_status: event.current,
    });
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
        .overwrite(true); // Not recommended for production. See `basic-infection/incidence-report`.;

    context.add_report::<IncidenceReportItem>(&parameters.output_file)?;
    context.subscribe_to_event(
        |context, event: PropertyChangeEvent<Person, InfectionStatus>| {
            handle_infection_status_change(context, event);
        },
    );
    Ok(())
}
