use ixa::context::Context;
use std::thread;

fn main() {
    let scenarios = vec!["Illinois", "Wisconsin", "Arizona", "California"];
    let mut handles = vec![];

    for scenario in scenarios {
        let scenario = scenario.to_string();
        let handle = thread::spawn(move || {
            // Replace this with the actual example code, similar to the simple
            // example in examples/reports/main.rs
            let mut context = Context::new();
            println!("Scenario: {}", scenario);

            context.execute();
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.join().unwrap();
    }
}
