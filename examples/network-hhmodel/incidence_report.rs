use std::fs;
use std::path::PathBuf;

use ixa::prelude::*;
use serde::{Deserialize, Serialize};

use crate::parameters::Parameters;
use crate::seir::{DiseaseStatus, InfectedBy};
use crate::Person;

#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
struct IncidenceReportItem {
    time: f64,
    person_id: String,
    infection_status: DiseaseStatus,
    infected_by: String,
}

define_report!(IncidenceReportItem);

fn handle_infection_status_change(
    context: &mut Context,
    event: PropertyChangeEvent<Person, DiseaseStatus>,
) {
    // check event to make sure it's a new infection
    if !(event.current == DiseaseStatus::E && event.previous == DiseaseStatus::S) {
        return;
    }

    // figure out who infected whom
    let infected_by: InfectedBy = context.get_property(event.entity_id);
    let infected_by_val = match infected_by.0 {
        None => "NA".to_string(),
        Some(id) => id.to_string(),
    };

    context.send_report(IncidenceReportItem {
        time: context.get_current_time(),
        person_id: event.entity_id.to_string(),
        infection_status: event.current,
        infected_by: infected_by_val,
    });
}

pub fn init(context: &mut Context) -> Result<(), IxaError> {
    let parameters = context.get_global_property_value(Parameters).unwrap();

    // Output directory is relative to the directory with the Cargo.toml file.
    let mut output_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    output_dir = output_dir.join(parameters.output_dir.clone());
    fs::create_dir_all(&output_dir)?;
    context
        .report_options()
        .directory(output_dir)
        .overwrite(true);
    context.add_report::<IncidenceReportItem>("incidence.csv")?;
    context.subscribe_to_event(
        |context: &mut Context, event: PropertyChangeEvent<Person, DiseaseStatus>| {
            handle_infection_status_change(context, event);
        },
    );
    Ok(())
}

#[cfg(test)]
mod test {
    use std::cell::RefCell;
    use std::path::Path;
    use std::rc::Rc;

    use super::*;
    use crate::parameters::ParametersValues;
    use crate::{incidence_report, loader, network, seir, MainRng, PersonId};

    fn check_values(path: &Path) -> (Vec<String>, Vec<IncidenceReportItem>) {
        let mut infected_by: Vec<IncidenceReportItem> = Vec::new();

        let mut rdr = csv::Reader::from_path(path).unwrap();
        let headers = rdr.headers().unwrap();
        let column_names = headers.iter().map(|s| s.to_string()).collect();
        for result in rdr.deserialize() {
            let record: IncidenceReportItem = result.unwrap();
            infected_by.push(record);
        }

        (column_names, infected_by)
    }

    fn test_infected_by(
        context: &mut Context,
        event: PropertyChangeEvent<Person, DiseaseStatus>,
        infected_by_out: &Rc<RefCell<Vec<IncidenceReportItem>>>,
    ) {
        // check event to make sure it's a new infection
        if !(event.current == DiseaseStatus::E && event.previous == DiseaseStatus::S) {
            return;
        }

        let infected_by: InfectedBy = context.get_property(event.entity_id);
        let infected_by_val = match infected_by.0 {
            None => "NA".to_string(),
            Some(id) => id.to_string(),
        };

        // save to a reportItem
        let infected_by_entry = IncidenceReportItem {
            time: context.get_current_time(),
            person_id: event.entity_id.to_string(),
            infection_status: event.current,
            infected_by: infected_by_val,
        };

        // add to the vec
        infected_by_out.borrow_mut().push(infected_by_entry)
    }

    #[test]
    fn test_incidence_report() {
        let mut output_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        output_dir = output_dir.join("examples/network-hhmodel/output".to_owned().clone());
        let parameters = ParametersValues {
            incubation_period: 8.0,
            infectious_period: 27.0,
            sar: 1.0,
            shape: 15.0,
            infection_duration: 5.0,
            between_hh_transmission_reduction: 1.0,
            data_dir: output_dir.to_str().unwrap().to_string(),
            output_dir: output_dir.to_str().unwrap().to_string(),
        };

        let infected_by_out: Rc<RefCell<Vec<IncidenceReportItem>>> =
            Rc::new(RefCell::new(Vec::new()));
        let infected_by_copy = Rc::clone(&infected_by_out);

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

            context.subscribe_to_event(
                move |context: &mut Context, event: PropertyChangeEvent<Person, DiseaseStatus>| {
                    test_infected_by(context, event, &infected_by_copy);
                },
            );

            let to_infect: Vec<PersonId> = vec![context.sample_entity(MainRng, Person).unwrap()];

            #[allow(clippy::vec_init_then_push)]
            seir::init(&mut context, &to_infect);

            context.execute();
        }

        let path = Path::new(&parameters.output_dir);
        assert!(path.try_exists().unwrap());
        let output_path = path.join("incidence.csv");
        assert!(output_path.try_exists().unwrap());

        let (column_names, report_results) = check_values(&output_path);

        assert_eq!(
            column_names,
            vec!["time", "person_id", "infection_status", "infected_by"]
        );

        assert_eq!(report_results, infected_by_out.borrow().clone());
    }
}
