use ixa::people::PersonPropertyChangeEvent;
use ixa::prelude::*;
use std::path::PathBuf;

use crate::InfectionStatus;
use crate::InfectionStatusValue;
use serde::{Deserialize, Serialize};

use crate::Parameters;

#[derive(Serialize, Deserialize, Clone)]
struct IncidenceReportItem {
    time: f64,
    person_id: String,
    infection_status: InfectionStatusValue,
}

define_report!(IncidenceReportItem);

fn handle_infection_status_change(
    context: &mut Context,
    event: PersonPropertyChangeEvent<InfectionStatus>,
) {
    context.send_report(IncidenceReportItem {
        time: context.get_current_time(),
        person_id: format!("{}", event.person_id),
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
    context.subscribe_to_event(
        |context, event: PersonPropertyChangeEvent<InfectionStatus>| {
            handle_infection_status_change(context, event);
        },
    );
    Ok(())
}
