use crate::infection_manager::InfectionStatusEvent;
use crate::people::InfectionStatusValue;
use ixa::context::Context;
use ixa::error::IxaError;
use ixa::report::ContextReportExt;
use ixa::report::Report;
use ixa::{create_report_trait, PersonId};
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
    context.send_report(IncidenceReportItem {
        time: context.get_current_time(),
        person_id: event.person_id,
        infection_status: event.current,
    });
}

pub fn init(context: &mut Context) -> Result<(), IxaError> {
    // In the configuration of report options below, we set `overwrite(true)`, which is not
    // recommended for production code in order to prevent accidental data loss. It is set
    // here so that newcomers won't have to deal with a confusing error while running
    // examples.
    context
        .report_options()
        .directory(PathBuf::from("./examples/basic-infection/"))
        .overwrite(true);
    context.add_report::<IncidenceReportItem>("incidence")?;
    context.subscribe_to_event::<InfectionStatusEvent>(handle_infection_status_change);
    Ok(())
}
