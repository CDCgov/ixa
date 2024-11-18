use ixa::context::Context;
use ixa::error::IxaError;
use ixa::global_properties::ContextGlobalPropertiesExt;
use ixa::people::ContextPeopleExt;
use ixa::report::ContextReportExt;
use ixa::{create_report_trait, report::Report};
use std::path::PathBuf;
use strum::IntoEnumIterator;

use serde::{Deserialize, Serialize};

use crate::parameters_loader::Parameters;
use crate::population_loader::{DiseaseStatus, DiseaseStatusType};

#[derive(Serialize, Deserialize, Clone)]
struct PeriodicReportItem {
    day: f64,
    // use a string because eventually we want
    // to be able to report on any property
    // and want to be able to use this one
    // struct for all properties, even those of
    // different types
    property_value: String,
    count: usize,
}

create_report_trait!(PeriodicReportItem);

fn count_people_and_send_report(context: &mut Context, report_period: f64) {
    for disease_state in DiseaseStatus::iter() {
        let mut counter = 0;
        for usize_id in 0..context.get_current_population() {
            if context.get_person_property(context.get_person_id(usize_id), DiseaseStatusType)
                == disease_state
            {
                counter += 1;
            }
        }
        context.send_report(PeriodicReportItem {
            day: context.get_current_time(),
            // format macro returns a string which is what
            // PeriodicReportItem struct expects
            // for generality across properties
            property_value: format!("{disease_state:?}"),
            count: counter,
        });
    }
    context.add_plan(context.get_current_time() + report_period, move |context| {
        count_people_and_send_report(context, report_period);
    });
}

pub fn init(context: &mut Context) -> Result<(), IxaError> {
    let parameters = context
        .get_global_property_value(Parameters)
        .unwrap()
        .clone();
    context
        .report_options()
        .directory(PathBuf::from(parameters.output_dir));
    context.add_report::<PeriodicReportItem>("person_property_count")?;
    count_people_and_send_report(context, parameters.report_period);
    Ok(())
}
