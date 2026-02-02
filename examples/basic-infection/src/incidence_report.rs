use std::path::PathBuf;

use ixa::prelude::*;
use ixa::serde::Serialize;
use ixa::trace;

use crate::infection_manager::InfectionStatusEvent;
use crate::people::{InfectionStatus, PersonId};

#[derive(Serialize)]
struct IncidenceReportItem {
    time: f64,
    person_id: PersonId,
    infection_status: InfectionStatus,
}

define_report!(IncidenceReportItem);

fn handle_infection_status_change(context: &mut Context, event: InfectionStatusEvent) {
    trace!(
        "Handling infection status change from {:?} to {:?} for {:?}",
        event.previous,
        event.current,
        event.entity_id
    );
    context.send_report(IncidenceReportItem {
        time: context.get_current_time(),
        person_id: event.entity_id,
        infection_status: event.current,
    });
}

pub fn init(context: &mut Context) -> Result<(), IxaError> {
    trace!("Initializing incidence_report");

    // Output directory is relative to the directory with the Cargo.toml file.
    let output_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("output");

    // In the configuration of report options below, we set `overwrite(true)`, which is not
    // recommended for production code in order to prevent accidental data loss. It is set
    // here so that newcomers won't have to deal with a confusing error while running
    // examples.
    context
        .report_options()
        .directory(output_path)
        .overwrite(true);
    context.add_report::<IncidenceReportItem>("incidence")?;
    context.subscribe_to_event::<InfectionStatusEvent>(handle_infection_status_change);
    Ok(())
}
