use clap::Args;
use ixa::runner::run_with_custom_args;

#[derive(Args, Debug)]
struct Extra {
    #[arg(short, long)]
    foo: bool,
}

fn main() {
    // Try running this with `cargo run --example runner -- --seed 42`
    run_with_custom_args(|context, args, extra: Option<Extra>| {
        context.add_plan(1.0, |_| {
            println!("Hello, world!");
        });
        println!("{}", args.random_seed);
        if let Some(extra) = extra {
            println!("{}", extra.foo);
        }
        Ok(())
    })
    .unwrap();
}
