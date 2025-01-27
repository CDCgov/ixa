use crate::context::Context;
use crate::debugger::ContextDebugExt;
use crate::error::IxaError;
use crate::global_properties::ContextGlobalPropertiesExt;
use crate::people::ContextPeopleExt;
use clap::{Parser, Subcommand};
use serde::{Deserialize, Serialize};

pub(crate) trait Extension {
    type Args;
    type Retval;

    fn run(context: &mut Context, args: &Self::Args) -> Result<Self::Retval, IxaError>;
}

#[derive(Serialize, Deserialize)]
pub(crate) struct EmptyArgs {}

pub(crate) fn run_extension<T: Extension>(
    context: &mut Context,
    args: &T::Args,
) -> Result<T::Retval, IxaError> {
    T::run(context, args)
}

pub(crate) struct PopulationExtension {}
#[derive(Parser, Debug, Deserialize)]
pub(crate) enum PopulationExtensionArgs {
    /// Get the total number of people
    Population,
}

#[derive(Serialize)]
pub(crate) struct PopulationExtensionRetval {
    pub population: usize,
}
impl Extension for PopulationExtension {
    type Args = EmptyArgs;
    type Retval = PopulationExtensionRetval;

    fn run(
        context: &mut Context,
        _args: &EmptyArgs,
    ) -> Result<PopulationExtensionRetval, IxaError> {
        Ok(PopulationExtensionRetval {
            population: context.get_current_population(),
        })
    }
}

pub(crate) struct GlobalPropertyExtension {}
#[derive(Serialize, Deserialize, Debug)]
pub(crate) enum GlobalPropertyExtensionRetval {
    List(Vec<String>),
    Value(String),
}
#[derive(Subcommand, Clone, Debug, Serialize, Deserialize)]
pub(crate) enum GlobalPropertyExtensionArgsEnum {
    /// List all global properties
    List,

    /// Get the value of a global property
    Get { property: String },
}
#[derive(Parser, Debug, Serialize, Deserialize)]
pub(crate) enum GlobalPropertyExtensionArgs {
    #[command(subcommand)]
    Global(GlobalPropertyExtensionArgsEnum),
}
impl Extension for GlobalPropertyExtension {
    type Args = GlobalPropertyExtensionArgs;
    type Retval = GlobalPropertyExtensionRetval;

    fn run(
        context: &mut Context,
        args: &GlobalPropertyExtensionArgs,
    ) -> Result<GlobalPropertyExtensionRetval, IxaError> {
        let GlobalPropertyExtensionArgs::Global(global_args) = args;

        match global_args {
            GlobalPropertyExtensionArgsEnum::List => Ok(GlobalPropertyExtensionRetval::List(
                context.list_registered_global_properties(),
            )),
            GlobalPropertyExtensionArgsEnum::Get { property: name } => {
                let output = context.get_serialized_value_by_string(name)?;
                match output {
                    Some(value) => Ok(GlobalPropertyExtensionRetval::Value(value)),
                    None => Err(IxaError::IxaError(format!("Property {name} is not set"))),
                }
            }
        }
    }
}

#[derive(Parser, Debug, Deserialize)]
pub(crate) enum NextExtensionArgs {
    /// Continue until the given time and then pause again
    Next { next_time: f64 },
}
#[derive(Serialize)]
pub(crate) struct NextExtensionRetval {}
pub(crate) struct NextCommandExtension {}
impl Extension for NextCommandExtension {
    type Args = NextExtensionArgs;
    type Retval = NextExtensionRetval;

    fn run(
        context: &mut Context,
        args: &NextExtensionArgs,
    ) -> Result<NextExtensionRetval, IxaError> {
        let NextExtensionArgs::Next { next_time } = args;
        context.schedule_debugger(*next_time);
        Ok(NextExtensionRetval {})
    }
}
