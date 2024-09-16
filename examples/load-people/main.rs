use ixa::{context::Context, random::ContextRandomExt};
mod logger;
mod population_loader;
mod sir;
mod vaccine;

fn main() {
    let mut context = Context::new();

    context.init_random(42);

    // Sets up some event listeners on person creation and property changes
    logger::init(&mut context);

    // This sets up the DiseaseStatus person property and schedules infections/recoveries
    // when each person is created.
    sir::init(&mut context);

    // Load people from csv and set up some base properties
    population_loader::init(&mut context);

    context.execute();
}
