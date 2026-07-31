use ixa::prelude::*;
use ixa::runner::run_with_args;
use ixa::{debug, info, trace};

define_entity!(Person);

fn main() -> Result<(), Box<dyn std::error::Error>> {
    run_with_args(|context, _args, _| {
        let _: PersonId = context.add_entity(Person)?;
        let _: PersonId = context.add_entity(Person)?;
        let _: PersonId = context.add_entity(Person)?;

        trace!("A TRACE message");
        debug!("A DEBUG message");
        info!("An INFO message");
        Ok(())
    })?;
    Ok(())
}
