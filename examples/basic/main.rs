use ixa::run_with_args;

fn main() {
    run_with_args(|context, _, _| {
        context.add_plan(1.0, |context| {
            println!("The current time is {}", context.get_current_time());
        });
        Ok(())
    })
    .unwrap();
}
