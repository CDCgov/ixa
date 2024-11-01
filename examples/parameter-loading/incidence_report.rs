use ixa::context::Context;
use ixa::global_properties::ContextGlobalPropertiesExt;
use ixa::people::PersonPropertyChangeEvent;
use ixa::report::ContextReportExt;
use ixa::{create_report_trait, report::Report};
use std::path::PathBuf;

use crate::InfectionStatus;
use crate::InfectionStatusType;
use serde::{Deserialize, Serialize};

use crate::Parameters;

#[derive(Serialize, Deserialize, Clone)]
struct IncidenceReportItem {
    time: f64,
    person_id: String,
    infection_status: InfectionStatus,
}

create_report_trait!(IncidenceReportItem);

fn handle_infection_status_change(
    context: &mut Context,
    event: PersonPropertyChangeEvent<InfectionStatusType>,
) {
    context.send_report(IncidenceReportItem {
        time: context.get_current_time(),
        person_id: format!("{}", event.person_id),
        infection_status: event.current,
    });
}

pub fn init(context: &mut Context) {
    let parameters = context.get_global_property_value(Parameters).clone();
    let dir_creation_res = context
        .report_options()
        .directory(PathBuf::from(parameters.output_dir));
    match dir_creation_res {
        Ok(()) => {
            context.add_report::<IncidenceReportItem>(&parameters.output_file);
            context.subscribe_to_event(
                |context, event: PersonPropertyChangeEvent<InfectionStatusType>| {
                    handle_infection_status_change(context, event);
                },
            );
        }
        Err(ixa_error) => {
            println!("Error creating directory: {ixa_error}");
        }
    }
}
