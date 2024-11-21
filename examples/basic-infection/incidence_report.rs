use ixa::context::Context;
use ixa::error::IxaError;
use ixa::people::{PersonId, PersonPropertyChangeEvent};
use ixa::report::ContextReportExt;
use ixa::{create_report_trait, report::Report};
use std::path::PathBuf;

use crate::population_loader::{InfectionStatus, InfectionStatusType};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone)]
struct IncidenceReportItem {
    time: f64,
    person_id: PersonId,
    infection_status: InfectionStatus,
}

create_report_trait!(IncidenceReportItem);

fn handle_infection_status_change(
    context: &mut Context,
    event: PersonPropertyChangeEvent<InfectionStatusType>,
) {
    context.send_report(IncidenceReportItem {
        time: context.get_current_time(),
        person_id: event.person_id,
        infection_status: event.current,
    });
}

pub fn init(context: &mut Context) -> Result<(), IxaError> {
    context
        .report_options()
        .directory(PathBuf::from("./examples/basic-infection/"));
    context.add_report::<IncidenceReportItem>("incidence")?;
    context.subscribe_to_event::<PersonPropertyChangeEvent<InfectionStatusType>>(
        |context, event| {
            handle_infection_status_change(context, event);
        },
    );
    Ok(())
}
