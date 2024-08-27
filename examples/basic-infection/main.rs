use ixa::context::Context;
mod people;
use crate::people::PeopleContext;

static POPULATION: u64 = 100;

fn main() {
    let mut context = Context::new();

    for person_id in 0..POPULATION {
        context.create_new_person(person_id);
    }
    //context.add_plan(0.5, |context| {
    //        context.create_new_person(1)});
    context.execute();
}
