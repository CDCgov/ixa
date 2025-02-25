mod people;
mod transmission_manager;
mod infection_manager;

use ixa::{error, info, run_with_args, Context};

static POPULATION: u64 = 10;
static FORCE_OF_INFECTION: f64 = 0.1;
static MAX_TIME: f64 = 30.0;
static INFECTION_DURATION: f64 = 10.0;

fn main() {
    let result =
        run_with_args(|context: &mut Context, _args, _| {
            people::init(context);
            transmission_manager::init(context);
            infection_manager::init(context);
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
