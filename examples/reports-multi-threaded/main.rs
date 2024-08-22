use ixa::context::Context;
use std::thread;

struct Incidence {
    scenario: String,
    person_id: String,
    t: f64,
}

fn main() {
    let scenarios = vec!["Illinois", "Wisconsin", "Arizona", "California"];
    let mut handles = vec![];

    for scenario in scenarios {
        let scenario = scenario.to_string();
        let handle = thread::spawn(move || {
            let mut context = Context::new();

            #[cfg(feature = "reports")]
            context.add_report::<Incidence>("Incidence");

            println!("Scenario: {}", scenario);

            let people = vec!["1", "2", "3"];
            for person in people {
                let person = person.to_string();
                let scenario = scenario.clone();
                context.add_plan(1.0, {
                    move |context| {
                        #[cfg(feature = "reports")]
                        context.send_report(Incidence {
                            scenario: scenario,
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
