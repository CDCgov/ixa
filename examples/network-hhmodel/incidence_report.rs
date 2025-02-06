use ixa::context::Context;
use ixa::error::IxaError;
use ixa::global_properties::ContextGlobalPropertiesExt;
use ixa::people::PersonPropertyChangeEvent;
use ixa::report::ContextReportExt;
use ixa::ContextPeopleExt;
use ixa::{create_report_trait, report::Report};
use std::{fs, path::PathBuf};
use serde::{Deserialize, Serialize};

use crate::parameters::Parameters;
use crate::seir::{DiseaseStatus, DiseaseStatusValue, InfectedBy};

#[derive(Serialize, Deserialize, Clone)]
struct IncidenceReportItem {
    time: f64,
    person_id: String,
    infection_status: DiseaseStatusValue,
    infected_by: String,
}

create_report_trait!(IncidenceReportItem);

fn handle_infection_status_change(
    context: &mut Context,
    event: PersonPropertyChangeEvent<DiseaseStatus>,
) {
    context.send_report(IncidenceReportItem {
        time: context.get_current_time(),
        person_id: event.person_id.to_string(),
        infection_status: event.current,
        infected_by: context
            .get_person_property(event.person_id, InfectedBy)
            .unwrap()
            .to_string(),
    });
}

pub fn init(context: &mut Context) -> Result<(), IxaError> {
    let _parameters = context
        .get_global_property_value(Parameters)
        .unwrap()
        .clone();
    fs::create_dir_all("examples/network-hhmodel/output")?;
    context
        .report_options()
        .directory(PathBuf::from("examples/network-hhmodel/output"))
        .overwrite(true);
    context.add_report::<IncidenceReportItem>("incidence.csv")?;
    context.subscribe_to_event(|context, event: PersonPropertyChangeEvent<DiseaseStatus>| {
        handle_infection_status_change(context, event);
    });
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::Path;
    use std::io::{self, BufRead};

    #[test]
    fn test_output_directory_created() {
        let mut context = Context::new();
        init(&mut context).unwrap();

        let output_path = Path::new("examples/network-hhmodel/output");
        assert!(output_path.exists());
        assert!(output_path.is_dir());

    }


}

