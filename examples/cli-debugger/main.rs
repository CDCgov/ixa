use ixa::runner::run_with_args;
use ixa::{
    define_person_property, define_person_property_with_default, ContextPeopleExt,
    PersonPropertyChangeEvent,
};

define_person_property_with_default!(IsRunner, bool, false);

// Using run_with_args / run_with_custom_args will parse command line options, including
// the --debugger interface.
// Try:
// cargo run --example cli-debugger -- --debugger      Starts the debugger before
//                                                     the simulation starts executing.
// cargo run --example cli-debugger -- --debugger 2.0  Pauses the simulation after all plans
//                                                     have executed for t=2.0
//
// When the debugger is open, type 'help' to see some options.
// Try typing `next` to step through each callback in this simulation
// (two plans, and an event listener when the status of a person changes)
fn main() {
    run_with_args(|context, _, _| {
        context.subscribe_to_event(|context, event: PersonPropertyChangeEvent<IsRunner>| {
            if !event.previous && event.current {
                println!(
                    "{} became a runner at t={}",
                    event.person_id,
                    context.get_current_time()
                );
            }
        });

        context.add_plan(2.0, |context| {
            println!("Adding person at t=2");
            let p1 = context.add_person(()).unwrap();

            context.add_plan(2.0, move |context| {
                println!("Change runner status, add another person at t=2");
                context.set_person_property(p1, IsRunner, true);
                context.add_person(()).unwrap();
            });
        });

        Ok(())
    })
    .unwrap();
}
