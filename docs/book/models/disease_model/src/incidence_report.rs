//ANCHOR: imports
use crate::{infection_manager::InfectionStatusEvent, people::InfectionStatusValue};
use csv;
use ixa::{prelude::*, trace, PersonId};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
//ANCHOR_END: imports

//ANCHOR: IncidenceReportItem
#[derive(Serialize, Deserialize, Clone)]
struct IncidenceReportItem {
    time: f64,
    person_id: PersonId,
    infection_status: InfectionStatusValue,
}
//ANCHOR_END: IncidenceReportItem

//ANCHOR: define_report
define_report!(IncidenceReportItem);
//ANCHOR_END: define_report

//ANCHOR: handle_infection_status_change
fn handle_infection_status_change(context: &mut Context, event: InfectionStatusEvent) {
    trace!(
        "Recording infection status change from {:?} to {:?} for {:?}",
        event.previous,
        event.current,
        event.person_id
    );
    context.send_report(IncidenceReportItem {
        time: context.get_current_time(),
        person_id: event.person_id,
        infection_status: event.current,
    });
}
//ANCHOR_END: handle_infection_status_change

// ANCHOR: init
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
// ANCHOR_END: init
