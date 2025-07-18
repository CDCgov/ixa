use ixa::people::PersonPropertyChangeEvent;
use ixa::prelude::*;
use std::path::Path;

use crate::population_manager::{
    Age, AgeGroupFoi, AgeGroupRisk, InfectionStatus, InfectionStatusValue,
};

use serde::{Deserialize, Serialize};

use crate::Parameters;

#[derive(Serialize, Deserialize, Clone)]
struct IncidenceReportItem {
    time: f64,
    person_id: String,
    age_group: AgeGroupRisk,
    age: u8,
    infection_status: InfectionStatusValue,
}

define_report!(IncidenceReportItem);

fn handle_infection_status_change(
    context: &mut Context,
    event: PersonPropertyChangeEvent<InfectionStatus>,
) {
    let age_person = context.get_person_property(event.person_id, Age);
    let age_group_person = context.get_person_property(event.person_id, AgeGroupFoi);
    context.send_report(IncidenceReportItem {
        time: context.get_current_time(),
        person_id: format!("{}", event.person_id),
        age_group: age_group_person,
        age: age_person,
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
        |context, event: PersonPropertyChangeEvent<InfectionStatus>| {
            handle_infection_status_change(context, event);
        },
    );
    Ok(())
}
