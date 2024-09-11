use ixa::context::Context;
use ixa::people::PersonCreatedEvent;
mod population_loader;

fn main() {
    let mut context = Context::new();

    // Load people from csv
    population_loader::init(&mut context);

    context.subscribe_to_event(move |_context, event: PersonCreatedEvent| {
        let person = event.person_id;
        println!("Person created with id {}", person.id);
    });

    context.execute();
}
