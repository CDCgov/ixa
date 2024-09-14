use ixa::{
    context::Context,
    people::{ContextPeopleExt, PersonCreatedEvent, PersonPropertyChangeEvent},
    random::ContextRandomExt,
};
use population_loader::Age;
use sir::DiseaseStatusType;
use vaccine::VaccineDoses;
mod population_loader;
mod sir;
mod vaccine;

fn main() {
    let mut context = Context::new();

    context.init_random(42);

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

    // Logs when a person is created
    context.subscribe_to_event(|context, event: PersonCreatedEvent| {
        let person = event.person_id;
        let age = context.get_person_property(person, Age);
        let vaccine_doses = context.get_person_property(person, VaccineDoses);
        println!(
            "Person {} age: {}, {} vaccine doses",
            person.id, age, vaccine_doses
        );
    });

    // This sets up the SIR person property and schedules infections/recoveries
    // When each person is created.
    sir::init(&mut context);

    // Load people from csv and set up some base properties
    // Note, this really has to come *after* anything that registers person
    // intiialization stuff.
    population_loader::init(&mut context);

    context.execute();
}
