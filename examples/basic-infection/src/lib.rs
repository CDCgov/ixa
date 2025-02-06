use ixa::context::Context;
use ixa::random::ContextRandomExt;

pub mod incidence_report;
pub mod infection_manager;
pub mod people;
pub mod transmission_manager;

static POPULATION: u64 = 1000;
static SEED: u64 = 123;
static MAX_TIME: f64 = 303.0;
static FOI: f64 = 0.1;
static INFECTION_DURATION: f64 = 5.0;

pub fn initialize(context: &mut Context) {
    context.init_random(SEED);

    people::init(context);
    transmission_manager::init(context);
    infection_manager::init(context);
    incidence_report::init(context).unwrap_or_else(|e| {
        eprintln!("failed to init incidence_report: {}", e);
    });
    context.add_plan(MAX_TIME, |context| {
        context.shutdown();
    });
}
