use crate::parameters::Parameters;
use crate::seir::{DiseaseStatus, DiseaseStatusValue, InfectedBy};
use ixa::people::PersonPropertyChangeEvent;
use ixa::prelude::*;
use ixa::report::Report;
use serde::{Deserialize, Serialize};
use std::{fs, path::PathBuf};

#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
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
    // check event to make sure it's a new infection
    if !(event.current == DiseaseStatusValue::E && event.previous == DiseaseStatusValue::S) {
        return;
    }

    // figure out who infected whom
    let infected_by_val = if context
        .get_person_property(event.person_id, InfectedBy)
        .is_none()
    {
        "NA".to_string()
    } else {
        context
            .get_person_property(event.person_id, InfectedBy)
            .unwrap()
            .to_string()
    };

    context.send_report(IncidenceReportItem {
        time: context.get_current_time(),
        person_id: event.person_id.to_string(),
        infection_status: event.current,
        infected_by: infected_by_val,
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
    use super::*;
    use crate::parameters::ParametersValues;
    use crate::{incidence_report, loader, network, seir, MainRng, PersonId};
    use std::{cell::RefCell, path::Path, rc::Rc};

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
        event: PersonPropertyChangeEvent<DiseaseStatus>,
        infected_by_out: &Rc<RefCell<Vec<IncidenceReportItem>>>,
    ) {
        // check event to make sure it's a new infection
        if !(event.current == DiseaseStatusValue::E && event.previous == DiseaseStatusValue::S) {
            return;
        }

        let infected_by_val = if context
            .get_person_property(event.person_id, InfectedBy)
            .is_none()
        {
            "NA".to_string()
        } else {
            context
                .get_person_property(event.person_id, InfectedBy)
                .unwrap()
                .to_string()
        };

        // save to a reportItem
        let infected_by_entry = IncidenceReportItem {
            time: context.get_current_time(),
            person_id: event.person_id.to_string(),
            infection_status: event.current,
            infected_by: infected_by_val,
        };

        // add to the vec
        infected_by_out.borrow_mut().push(infected_by_entry)
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
                move |context: &mut Context, event: PersonPropertyChangeEvent<DiseaseStatus>| {
                    test_infected_by(context, event, &infected_by_copy);
                },
            );

            let to_infect: Vec<PersonId> = vec![context.sample_person(MainRng, ()).unwrap()];

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
