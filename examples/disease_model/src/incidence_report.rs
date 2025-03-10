use crate::{infection_manager::InfectionStatusEvent, people::InfectionStatusValue};
use csv;
use ixa::{create_report_trait, trace, Context, ContextReportExt, IxaError, PersonId, Report};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Serialize, Deserialize, Clone)]
struct IncidenceReportItem {
    time: f64,
    person_id: PersonId,
    infection_status: InfectionStatusValue,
}

create_report_trait!(IncidenceReportItem);

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

pub fn init(context: &mut Context) -> Result<(), IxaError> {
    trace!("Initializing incidence_report");

    // In the configuration of report options below, we set `overwrite(true)`, which is not
    // recommended for production code in order to prevent accidental data loss. It is set
    // here so that newcomers won't have to deal with a confusing error while running
    // examples.
    context
        .report_options()
        .directory(PathBuf::from("../"))
        .overwrite(true);
    context.add_report::<IncidenceReportItem>("incidence")?;
    context.subscribe_to_event::<InfectionStatusEvent>(handle_infection_status_change);
    Ok(())
}
