use crate::population_manager::{
    VaccineAgeGroup,
    AgeGroupRisk,
    CensusTract
};

use crate::Parameters;
use ixa::{
    error::IxaError,
    context::Context,
    create_report_trait,
    global_properties::ContextGlobalPropertiesExt,
    define_data_plugin,
    people::{ContextPeopleExt, PersonCreatedEvent},
    report::{ContextReportExt, Report},
};
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::path::PathBuf;

use std::collections::HashSet;

use strum::IntoEnumIterator;

#[derive(Serialize, Deserialize, Clone, PartialEq)]
struct PersonReportItem {
    time: f64,
    age_group: AgeGroupRisk,
    population: usize,
    census_tract: usize,
}

#[derive(Clone)]
struct PopulationReportData {
    census_tract_set: HashSet<usize>,
}

define_data_plugin!(
    PopulationReportPlugin,
    PopulationReportData,
    PopulationReportData {
        census_tract_set: HashSet::new(),
    }
);

create_report_trait!(PersonReportItem);

fn build_property_groups(context: &mut Context, report_period: f64) {
    let population_data = context
        .get_data_container_mut(PopulationReportPlugin);

    let current_census_set = population_data
        .census_tract_set
        .clone();

    for age_group in AgeGroupRisk::iter() {
        for tract in &current_census_set{
            let age_group_pop = context
                .query_people(((VaccineAgeGroup, age_group), (CensusTract, (*tract).clone())))
                .len();

            context.send_report(PersonReportItem {
                time: context.get_current_time(),
                age_group: age_group,
                population: age_group_pop,
                census_tract: *tract
            });
        }
    }

    context.add_plan(context.get_current_time() + report_period, move |context| {
        build_property_groups(context, report_period);
    });
}

fn update_property_set(context: &mut Context, event: PersonCreatedEvent) {
    let person_census = context
        .get_person_property(event.person_id, CensusTract)
        .clone();
    let report_plugin = context
        .get_data_container_mut(PopulationReportPlugin);
    report_plugin.census_tract_set
        .insert(person_census);
}


pub fn init(context: &mut Context) -> Result<(), IxaError> {
    let parameters = context.get_global_property_value(Parameters)
        .unwrap()
        .clone();

    let current_dir = Path::new(file!()).parent().unwrap();
    context
        .report_options()
        .overwrite(true)
        .directory(PathBuf::from(current_dir));

    context.subscribe_to_event(
        |context, event: PersonCreatedEvent| {
            update_property_set(context, event);
        }
    );

    context.add_report::<PersonReportItem>(&parameters.output_file)?;
    context.add_plan(0.0, move |context| {
        build_property_groups(context, parameters.report_period);
    });
    Ok(())
}
