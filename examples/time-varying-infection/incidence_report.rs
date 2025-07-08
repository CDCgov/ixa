use ixa::people::PersonPropertyChangeEvent;
use ixa::prelude::*;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::parameters_loader::Parameters;
use crate::population_loader::{DiseaseStatus, DiseaseStatusValue};

#[derive(Serialize, Deserialize, Clone)]
struct IncidenceReportItem {
    time: f64,
    person_id: String,
    infection_status: DiseaseStatusValue,
}

define_report!(IncidenceReportItem);

fn handle_infection_status_change(
    context: &mut Context,
    event: PersonPropertyChangeEvent<DiseaseStatus>,
) {
    context.send_report(IncidenceReportItem {
        time: context.get_current_time(),
        person_id: event.person_id.to_string(),
        infection_status: event.current,
    });
}

pub fn init(context: &mut Context) -> Result<(), IxaError> {
    let parameters = context
        .get_global_property_value(Parameters)
        .unwrap()
        .clone();
    context
        .report_options()
        .directory(PathBuf::from(parameters.output_dir))
        .overwrite(true); // Not recommended for production. See `basic-infection/incidence-report`.;
    context.add_report::<IncidenceReportItem>(&parameters.output_file)?;
    context.subscribe_to_event(|context, event: PersonPropertyChangeEvent<DiseaseStatus>| {
        handle_infection_status_change(context, event);
    });
    Ok(())
}
