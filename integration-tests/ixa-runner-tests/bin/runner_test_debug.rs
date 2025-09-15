use ixa::prelude::*;
use ixa::runner::run_with_args;
use ixa::{debug, info, trace};

fn main() {
    run_with_args(|context, _args, _| {
        context.add_person(()).unwrap();
        context.add_person(()).unwrap();
        context.add_person(()).unwrap();

        trace!("A TRACE message");
        debug!("A DEBUG message");
        info!("An INFO message");
        Ok(())
    })
    .unwrap();
}
