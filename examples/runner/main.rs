use ixa::runner::run_with_args;

fn main() {
    // Try running this with `cargo run --example runner -- --seed 42`
    run_with_args(|context, args| {
        context.add_plan(1.0, |_| {
            println!("Hello, world!");
        });
        println!("{}", args.seed);
        Ok(())
    })
    .unwrap();
}
