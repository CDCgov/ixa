use ixa::{context::Context, random::ContextRandomExt};
mod loader;

fn main() {
    let mut context = Context::new();

    context.init_random(42);

    // Sets up some event listeners on person creation and property changes
    // logger::init(&mut context);

    // Load people from csv and set up some base properties
    loader::init(&mut context);

    // context.execute();
}
