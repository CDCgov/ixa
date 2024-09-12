use ixa::{context::Context, people::PersonPropertyChangeEvent};
use sir::DiseaseStatusType;
mod population_loader;
mod sir;

fn main() {
    let mut context = Context::new();

    // This sets up the SIR person property and schedules infections/recoveries
    // When each person is created.
    sir::init(&mut context);

    // Load people from csv and set up some base properties
    // Note, this really has to come *after* anything that registers person
    // intiialization stuff.
    population_loader::init(&mut context);

    // This subscribes to the disease status change events
    // Note that no event get fired when the property is set the first time
    context.subscribe_to_event(
        |_context, event: PersonPropertyChangeEvent<DiseaseStatusType>| {
            let person = event.person_id;
            println!(
                "Person {} changed disease status from {:?} to {:?}",
                person.id, event.previous, event.current,
            );
        },
    );

    context.execute();
}
