use ixa::prelude::*;

fn main() {
    run_with_args(|context, _args, _| {
        context.add_person(()).unwrap();
        context.add_person(()).unwrap();
        context.add_person(()).unwrap();

        Ok(())
    })
    .unwrap();
}
