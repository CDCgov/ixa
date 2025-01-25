use crate::context::Context;
use crate::debugger::{
    ContextDebugExt, GlobalPropertySubcommand, GlobalPropertySubcommandEnum, NextSubcommand,
};
use crate::error::IxaError;
use crate::global_properties::ContextGlobalPropertiesExt;
use crate::people::ContextPeopleExt;

pub(crate) trait Extension {
    type Args;
    type Retval;

    fn run(context: &mut Context, args: &Self::Args) -> Result<Self::Retval, IxaError>;
}

pub(crate) fn run_extension<T: Extension>(
    context: &mut Context,
    args: &T::Args,
) -> Result<T::Retval, IxaError> {
    T::run(context, args)
}

pub(crate) struct PopulationExtension {}
impl Extension for PopulationExtension {
    type Args = ();
    type Retval = usize;

    fn run(context: &mut Context, _args: &()) -> Result<usize, IxaError> {
        Ok(context.get_current_population())
    }
}

pub(crate) struct GlobalPropertyExtension {}
pub(crate) enum GlobalPropertyExtensionRetval {
    List(Vec<String>),
    Value(String),
}

impl Extension for GlobalPropertyExtension {
    type Args = GlobalPropertySubcommand;
    type Retval = GlobalPropertyExtensionRetval;

    fn run(
        context: &mut Context,
        args: &GlobalPropertySubcommand,
    ) -> Result<GlobalPropertyExtensionRetval, IxaError> {
        let GlobalPropertySubcommand::Global(global_args) = args;

        match global_args {
            GlobalPropertySubcommandEnum::List => Ok(GlobalPropertyExtensionRetval::List(
                context.list_registered_global_properties(),
            )),
            GlobalPropertySubcommandEnum::Get { property: name } => {
                let output = context.get_serialized_value_by_string(&name)?;
                match output {
                    Some(value) => Ok(GlobalPropertyExtensionRetval::Value(value)),
                    None => Err(IxaError::IxaError(format!("Property {name} is not set"))),
                }
            }
        }
    }
}

pub(crate) struct NextCommandExtension {}
impl Extension for NextCommandExtension {
    type Args = NextSubcommand;
    type Retval = ();

    fn run(context: &mut Context, args: &NextSubcommand) -> Result<(), IxaError> {
        let NextSubcommand::Next { next_time } = args;
        context.schedule_debugger(*next_time);
        Ok(())
    }
}
