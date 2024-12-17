use clap::Args;
use ixa::runner::run_with_custom_args;
use ixa::ContextPeopleExt;

#[derive(Args, Debug)]
struct CustomArgs {
    #[arg(short = 'p', long)]
    starting_population: Option<u8>,
}

fn main() {
    // Try running the following:
    // cargo run --example runner -- --seed 42
    // cargo run --example runner -- --starting-population 5
    // cargo run --example runner -- -p 5 --debugger
    let context = run_with_custom_args(|context, args, custom_args: Option<CustomArgs>| {
        println!("Setting random seed to {}", args.random_seed);

        // If an initial population was provided, add each person
        if let Some(custom_args) = custom_args {
            if let Some(population) = custom_args.starting_population {
                for _ in 0..population {
                    context.add_person(()).unwrap();
                }
            }
        }

        context.add_plan(2.0, |context| {
            println!("Adding two people at t=2");
            context.add_person(()).unwrap();
            context.add_person(()).unwrap();
        });

        Ok(())
    })
    .unwrap();

    let final_count = context.get_current_population();
    println!("Simulation complete. The number of people is: {final_count}");
}
