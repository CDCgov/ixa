use ixa::runner::run_with_args;
use ixa::ContextPeopleExt;
fn main() {
    run_with_args(|context, _args, _| {
        context.add_person(()).unwrap();
        context.add_person(()).unwrap();
        context.add_person(()).unwrap();

        Ok(())
    })
    .unwrap();
}
