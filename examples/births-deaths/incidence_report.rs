use ixa::context::Context;
use ixa::error::IxaError;
use ixa::global_properties::ContextGlobalPropertiesExt;
use ixa::people::{ContextPeopleExt, PersonPropertyChangeEvent};
use ixa::report::ContextReportExt;
use ixa::{create_report_trait, report::Report};
use std::path::Path;
use std::path::PathBuf;

use crate::population_manager::{
    Age, AgeGroupFoi, AgeGroupRisk, InfectionStatus, InfectionStatusValue,
};

use serde::{Deserialize, Serialize};

use crate::Parameters;

#[derive(Serialize, Deserialize, Clone)]
struct IncidenceReportItem {
    time: f64,
    person_id: String,
    age_group: AgeGroupRisk,
    age: u8,
    infection_status: InfectionStatusValue,
}

create_report_trait!(IncidenceReportItem);

fn handle_infection_status_change(
    context: &mut Context,
    event: PersonPropertyChangeEvent<InfectionStatus>,
) {
    let age_person = context.get_person_property(event.person_id, Age);
    let age_group_person = context.get_person_property(event.person_id, AgeGroupFoi);
    context.send_report(IncidenceReportItem {
        time: context.get_current_time(),
        person_id: format!("{}", event.person_id),
        age_group: age_group_person,
        age: age_person,
        infection_status: event.current,
    });
}

pub fn init(context: &mut Context) -> Result<(), IxaError> {
    let parameters = context
        .get_global_property_value(Parameters)
        .unwrap()
        .clone();
    let current_dir = Path::new(file!()).parent().unwrap();
    context
        .report_options()
        .directory(PathBuf::from(current_dir))
        .overwrite(true); // Not recommended for production. See `basic-infection/incidence-report`.;

    context.add_report::<IncidenceReportItem>(&parameters.output_file)?;
    context.subscribe_to_event(
        |context, event: PersonPropertyChangeEvent<InfectionStatus>| {
            handle_infection_status_change(context, event);
        },
    );
    Ok(())
}
