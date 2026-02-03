use clap::Args;
use ixa::prelude::*;
use ixa::runner::run_with_custom_args;

#[derive(Args, Debug)]
struct CustomArgs {
    // Example of a boolean argument.
    // --say-hello       custom_args.say_hello is true
    // (nothing)         custom_args.say_hello is false
    #[arg(long)]
    say_hello: bool,

    // Example of an optional argument with a required value
    // -p 12            custom_args.starting_population is Some(12)
    // -p               This is invalid; you have to pass a value.
    // (nothing)        custom_args.starting_population is None
    #[arg(short = 'p', long)]
    starting_population: Option<u8>,
}

define_global_property!(Name, String);

define_entity!(Person);

fn main() {
    // The runner reads arguments from the command line.
    // args refer to `BaseArgs` (see runner.rs for all available args)
    // custom_args are optional for any arguments you want to define yourself.
    //
    // Try running the following:
    // cargo run --example runner -- --seed 42
    // cargo run --example runner -- --starting-population 5
    // cargo run --example runner -- -p 5 --debugger
    let context = run_with_custom_args(|context, args, custom_args: Option<CustomArgs>| {
        println!("Setting random seed to {}", args.random_seed);

        // As long as you specified a custom type in the signature (CustomArgs),
        // you should assume custom_args is Some (even if no args were passed
        // through the command line). It's None if you didn't specify any custom type.
        let custom_args = custom_args.unwrap();

        if custom_args.say_hello {
            println!("Hello");
        }

        context.set_global_property_value(Name, "Sim123".to_string())?;

        println!("Name: {}", context.get_global_property_value(Name).unwrap());

        if let Some(population) = custom_args.starting_population {
            for _ in 0..population {
                let _: PersonId = context.add_entity(()).unwrap();
            }
        }

        context.add_plan(2.0, |context| {
            println!("Adding two people at t=2");
            let _: PersonId = context.add_entity(()).unwrap();
            let _: PersonId = context.add_entity(()).unwrap();
        });

        Ok(())
    })
    .unwrap();

    let final_count = context.get_entity_count::<Person>();
    println!("Simulation complete. The number of people is: {final_count}");
}
