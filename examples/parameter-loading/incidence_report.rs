use ixa::context::Context;
use ixa::report::ContextReportExt;
use ixa::{create_report_trait, report::Report};
use std::path::PathBuf;

use crate::people::InfectionStatus;
use crate::people::InfectionStatusEvent;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone)]
struct IncidenceReportItem {
    time: f64,
    person_id: usize,
    infection_status: InfectionStatus,
}

create_report_trait!(IncidenceReportItem);

fn handle_infection_status_change(context: &mut Context, event: InfectionStatusEvent) {
    context.send_report(IncidenceReportItem {
        time: context.get_current_time(),
        person_id: event.person_id,
        infection_status: event.updated_status,
    });
}

pub fn init(context: &mut Context) {
    context
        .report_options()
        .directory(PathBuf::from("./examples/parameter-loading/"));
    context.add_report::<IncidenceReportItem>("incidence");
    context.subscribe_to_event::<InfectionStatusEvent>(|context, event| {
        handle_infection_status_change(context, event);
    });
}
