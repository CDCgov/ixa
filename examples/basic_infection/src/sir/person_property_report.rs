use clap::Arg;
use eosim::{
    context::{Component, Context},
    people::PersonId,
    person_properties::PersonPropertyContext,
    reports::{Report, ReportsContext},
};
use serde_derive::Serialize;

use super::person_properties::DiseaseStatus;
use super::person_properties::HealthStatus;

pub struct PersonPropertyReport {}

#[derive(Serialize, PartialEq, Debug)]
pub struct PersonPropertyChange {
    pub time: f64,
    pub person_property: String,
    pub value: String,
}

impl Report for PersonPropertyReport {
    type Item = PersonPropertyChange;
}

pub fn handle_person_disease_status_change(
    context: &mut Context,
    person_id: PersonId,
    _: DiseaseStatus,
) {
    let disease_status = context.get_person_property_value::<DiseaseStatus>(person_id);
    context.release_report_item::<PersonPropertyReport>(PersonPropertyChange {
        time: context.get_time(),
        person_property: "DiseaseStatus".to_owned(),
        value: format!("{:?}", disease_status),
    })
}

pub fn handle_person_health_status_change(
    context: &mut Context,
    person_id: PersonId,
    _: HealthStatus,
) {
    let health_status = context.get_person_property_value::<HealthStatus>(person_id);
    context.release_report_item::<PersonPropertyReport>(PersonPropertyChange {
        time: context.get_time(),
        person_property: "HealthStatus".to_owned(),
        value: format!("{:?}", health_status),
    })
}

impl Component for PersonPropertyReport {
    fn init(context: &mut Context) {
        context
            .observe_person_property_changes::<DiseaseStatus>(handle_person_disease_status_change);
        context.observe_person_property_changes::<HealthStatus>(handle_person_health_status_change);
    }
}

#[cfg(test)]
mod test {
    /*
    - Set up data needed for test
     - Create 2 persons, change their properties from S to E and report
    - Run code
    - Compare results with expectations
     */
    use super::*;
    use eosim::{
        context::Context,
        people::PersonId,
        person_properties::PersonPropertyContext,
        reports::{get_file_report_handler, Report, ReportsContext},
    };
    use std::io::{Read, Seek};
    use tempfile::tempfile;

    #[test]
    fn check_health_report() {
        let mut context = Context::new();
        let id = 0;
        let person_id_test = PersonId::new(id);
        let output_file = tempfile().unwrap();
        //let mut output_file2 = File::create("person_property_report_test.csv").unwrap();
        context.set_report_item_handler::<PersonPropertyReport>(get_file_report_handler::<
            PersonPropertyReport,
        >(
            output_file.try_clone().unwrap()
        ));
        context.set_person_property_value::<DiseaseStatus>(person_id_test, DiseaseStatus::S);
        context.add_plan(1.0, move |context| {
            context.set_person_property_value::<DiseaseStatus>(person_id_test, DiseaseStatus::I);
        });

        context
            .observe_person_property_changes::<DiseaseStatus>(handle_person_disease_status_change);

        context.execute();
        let disease_status = context.get_person_property_value::<DiseaseStatus>(person_id_test);

        drop(context);
        let mut output_file = output_file.try_clone().unwrap();
        output_file.rewind().unwrap();
        let mut string = String::new();
        output_file.read_to_string(&mut string).unwrap();
        assert_eq!(disease_status, DiseaseStatus::I, "Should equal I");
        assert_eq!(string, "time,person_property,value\n1.0,DiseaseStatus,I\n");
    }

    #[test]
    fn check_dummy_report() -> Result<(), String> {
        let result = 1 + 1;
        if result == 2 {
            Ok(())
        } else {
            Err(String::from("should equal 2"))
        }
        //assert_eq!(result, 2, "Sum does not match");
    }
}
