use ixa::context::Context;
use ixa::report::ContextReportExt;
use ixa::{create_report_trait, report::Report};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::thread;

#[allow(dead_code)]
#[derive(Serialize, Deserialize, Clone)]
struct Incidence {
    scenario: String,
    person_id: String,
    t: f64,
}

create_report_trait!(Incidence);

#[allow(unexpected_cfgs)]
fn main() {
    let scenarios = vec!["Illinois", "Wisconsin", "Arizona", "California"];
    let mut handles = vec![];

    for scenario in scenarios {
        let scenario = scenario.to_string();
        let handle = thread::spawn(move || {
            let mut context = Context::new();

            context
                .report_options()
                .directory(PathBuf::from("./examples/reports-multi-threaded"))
                .file_prefix(format!("{scenario}_"));
            context
                .add_report::<Incidence>("incidence")
                .expect("Error adding report");
            println!("Scenario: {scenario}");

            let people = vec!["1", "2", "3"];
            for person in people {
                let person = person.to_string();
                let scenario = scenario.clone();
                context.add_plan(1.0, {
                    move |context| {
                        context.send_report(Incidence {
                            scenario: scenario.to_string(),
                            person_id: person.clone(),
                            t: context.get_current_time(),
                        });
                        println!(
                            "Scenario: {}, Person {} was infected at time {}.",
                            scenario,
                            person,
                            context.get_current_time()
                        );
                    }
                });
            }

            context.execute();
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.join().unwrap();
    }
}
