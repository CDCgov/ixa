// ANCHOR: header
mod incidence_report;
mod infection_manager;
mod people;
mod transmission_manager;

use ixa::{error, info, run_with_args, Context};

static POPULATION: u64 = 100;
static FORCE_OF_INFECTION: f64 = 0.1;
static INFECTION_DURATION: f64 = 10.0;
static MAX_TIME: f64 = 200.0;
// ANCHOR_END: header

fn main() {
    let result = run_with_args(|context: &mut Context, _args, _| {
        // Add a plan to shut down the simulation after `max_time`, regardless of
        // what else is happening in the model.
        context.add_plan(MAX_TIME, |context| {
            context.shutdown();
        });
        people::init(context);
        transmission_manager::init(context);
        infection_manager::init(context);
        incidence_report::init(context).expect("Failed to init incidence report");
        Ok(())
    });

    match result {
        Ok(_) => {
            info!("Simulation finished executing");
        }
        Err(e) => {
            error!("Simulation exited with error: {}", e);
        }
    }
}
