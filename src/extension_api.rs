use crate::context::Context;
use crate::debugger::PopulationSubcommand;
use crate::people::ContextPeopleExt;
use std::{any::Any, collections::HashMap};

pub(crate) trait Extension {
    type Args;
    type Result;

    fn run(context: &mut Context, args: &Self::Args) -> Self::Result;
}

pub(crate) fn run_extension<T: Extension>(context: &mut Context, args: &T::Args) -> T::Result {
    T::run(context, args)
}

pub(crate) struct PopulationExtension {}
impl Extension for PopulationExtension {
    type Args = ();
    type Result = usize;

    fn run(context: &mut Context, _args: &()) -> usize {
        context.get_current_population()
    }
}
