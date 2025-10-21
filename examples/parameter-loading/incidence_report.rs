use std::path::PathBuf;

use ixa::people::PersonPropertyChangeEvent;
use ixa::prelude::*;
use serde::{Deserialize, Serialize};

use crate::{InfectionStatus, InfectionStatusValue, Parameters};

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
    // Output directory is relative to the directory with the Cargo.toml file.
    let mut output_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    output_dir = output_dir.join(parameters.output_dir.clone());
    context
        .report_options()
        .directory(output_dir)
        .overwrite(true); // Not recommended for production. See `basic-infection/incidence-report`.;
    context.add_report::<IncidenceReportItem>(&parameters.output_file)?;
    context.subscribe_to_event(
        |context, event: PersonPropertyChangeEvent<InfectionStatus>| {
            handle_infection_status_change(context, event);
        },
    );
    Ok(())
}
