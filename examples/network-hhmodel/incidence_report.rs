use crate::parameters::Parameters;
use crate::seir::{DiseaseStatus, DiseaseStatusValue, InfectedBy};
use ixa::context::Context;
use ixa::error::IxaError;
use ixa::global_properties::ContextGlobalPropertiesExt;
use ixa::people::PersonPropertyChangeEvent;
use ixa::report::ContextReportExt;
use ixa::ContextPeopleExt;
use ixa::{create_report_trait, report::Report};
use serde::{Deserialize, Serialize};
use std::{fs, path::PathBuf};

#[derive(Serialize, Deserialize, Clone, Debug)]
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
    if !(event.current == DiseaseStatusValue::E && event.previous == DiseaseStatusValue::S) {
        return;
    }

    context.send_report(IncidenceReportItem {
        time: context.get_current_time(),
        person_id: event.person_id.to_string(),
        infection_status: event.current,
        infected_by: context
            .get_person_property(event.person_id, InfectedBy)
            .expect("Expected person to have infectedBy but none was found")
            .to_string(),
    });
}

pub fn init(context: &mut Context) -> Result<(), IxaError> {
    let parameters = context.get_global_property_value(Parameters).unwrap();
    let output_dir = PathBuf::from(parameters.output_dir.clone());
    fs::create_dir_all(&output_dir)?;
    context
        .report_options()
        .directory(output_dir)
        .overwrite(true);
    context.add_report::<IncidenceReportItem>("incidence.csv")?;
    context.subscribe_to_event(
        |context: &mut Context, event: PersonPropertyChangeEvent<DiseaseStatus>| {
            handle_infection_status_change(context, event);
        },
    );
    Ok(())
}

#[cfg(test)]
mod test {
    use super::IncidenceReportItem;
    use super::*;
    use crate::parameters::ParametersValues;
    use crate::{incidence_report, loader, network, seir, MainRng, PersonId};
    use ixa::{context::Context, random::ContextRandomExt, ContextPeopleExt};
    use std::cell::RefCell;
    use std::collections::HashMap;
    use std::path::Path;
    use std::rc::Rc;

    fn read_csv_column_names(path: &Path) -> Result<Vec<String>, IxaError> {
        let mut rdr = csv::Reader::from_path(path)?;
        let headers = rdr.headers()?;
        let column_names = headers.iter().map(|s| s.to_string()).collect();
        Ok(column_names)
    }

    fn check_values(path: &Path) -> HashMap<String, String> {
        let mut infected_by: HashMap<String, String> = HashMap::new();

        let mut rdr = csv::Reader::from_path(path).unwrap();
        let headers = rdr.headers().unwrap();
        for result in rdr.deserialize() {
            let record: IncidenceReportItem = result.unwrap();
            infected_by.insert(record.person_id, record.infected_by);
        }

        infected_by
    }

    #[test]
    fn test_incidence_report() {
        let parameters = ParametersValues {
            incubation_period: 8.0,
            infectious_period: 27.0,
            sar: 1.0,
            shape: 15.0,
            infection_duration: 5.0,
            between_hh_transmission_reduction: 1.0,
            data_dir: "examples/network-hhmodel/tests".to_owned(),
            output_dir: "examples/network-hhmodel/tests".to_owned(),
        };

        let infected_by: Rc<RefCell<HashMap<String, String>>> =
            Rc::new(RefCell::new(HashMap::new()));
        let infected_by_copy = Rc::clone(&infected_by);

        // We need to put this code where we actually set up the model and write to the report in
        // its own scope so that the report flushes at scope close, allowing us to read the values
        // in it.
        {
            let mut context = Context::new();

            context.init_random(2);

            context
                .set_global_property_value(Parameters, parameters.clone())
                .unwrap();

            let people = loader::init(&mut context);
            network::init(&mut context, &people);
            incidence_report::init(&mut context).unwrap();
            let to_infect = context.sample_person(MainRng, ()).unwrap();
            context.set_person_property(to_infect, InfectedBy, Some(to_infect));
            infected_by
                .borrow_mut()
                .insert(format!("{to_infect}"), format!("{to_infect}"));
            context.subscribe_to_event(
                move |context: &mut Context, event: PersonPropertyChangeEvent<DiseaseStatus>| {
                    infected_by_copy.borrow_mut().insert(
                        format!("{}", event.person_id),
                        format!(
                            "{}",
                            context
                                .get_person_property(event.person_id, InfectedBy)
                                .unwrap()
                        ),
                    );
                },
            );
            #[allow(clippy::vec_init_then_push)]
            seir::init(&mut context, &vec![to_infect]);
            context.execute();
        }
        let path = Path::new(&parameters.output_dir);
        assert!(path.try_exists().unwrap());
        let output_path = path.join("incidence.csv");
        assert!(output_path.try_exists().unwrap());

        let column_names = read_csv_column_names(&output_path).unwrap();

        assert_eq!(
            column_names,
            vec!["time", "person_id", "infection_status", "infected_by"]
        );

        let report_results = check_values(&output_path);
        assert_eq!(report_results, infected_by.borrow().clone());
    }
}
