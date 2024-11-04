use ixa::context::Context;
use ixa::global_properties::ContextGlobalPropertiesExt;
use ixa::people::PersonPropertyChangeEvent;
use ixa::report::ContextReportExt;
use ixa::{create_report_trait, report::Report};
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::parameters_loader::Parameters;
use crate::population_loader::{DiseaseStatus, DiseaseStatusType};

#[derive(Serialize, Deserialize, Clone)]
struct IncidenceReportItem {
    time: f64,
    person_id: String,
    infection_status: DiseaseStatus,
}

create_report_trait!(IncidenceReportItem);

fn handle_infection_status_change(
    context: &mut Context,
    event: PersonPropertyChangeEvent<DiseaseStatusType>,
) {
    context.send_report(IncidenceReportItem {
        time: context.get_current_time(),
        person_id: event.person_id.to_string(),
        infection_status: event.current,
    });
}

pub fn init(context: &mut Context) {
    let parameters = context.get_global_property_value(Parameters).unwrap().clone();
    context
        .report_options()
        .directory(PathBuf::from(parameters.output_dir));
    context.add_report::<IncidenceReportItem>(&parameters.output_file);
    context.subscribe_to_event(
        |context, event: PersonPropertyChangeEvent<DiseaseStatusType>| {
            handle_infection_status_change(context, event);
        },
    );
}
