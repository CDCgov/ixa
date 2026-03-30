use ixa::prelude::*;
use ixa::runner::run_with_args;
use ixa::{debug, info, trace};

define_entity!(Person);

fn main() {
    run_with_args(|context, _args, _| {
        let _: PersonId = context.add_entity(()).unwrap();
        let _: PersonId = context.add_entity(()).unwrap();
        let _: PersonId = context.add_entity(()).unwrap();

        trace!("A TRACE message");
        debug!("A DEBUG message");
        info!("An INFO message");
        Ok(())
    })
    .unwrap();
}
