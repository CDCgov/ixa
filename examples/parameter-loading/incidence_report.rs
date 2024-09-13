use ixa::context::Context;
use ixa::global_properties::ContextGlobalPropertiesExt;
use ixa::report::ContextReportExt;
use ixa::{create_report_trait, report::Report};
use std::path::PathBuf;

use crate::people::InfectionStatus;
use crate::people::InfectionStatusEvent;
use serde::{Deserialize, Serialize};

use crate::Parameters;

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
    let parameters = context.get_global_property_value(Parameters).clone();
    context
        .report_options()
        .directory(PathBuf::from(parameters.output_dir));
    context.add_report::<IncidenceReportItem>(&parameters.output_file);
    context.subscribe_to_event::<InfectionStatusEvent>(|context, event| {
        handle_infection_status_change(context, event);
    });
}
