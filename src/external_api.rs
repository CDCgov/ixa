use crate::context::Context;
use crate::debugger::ContextDebugExt;
use crate::error::IxaError;
use crate::global_properties::ContextGlobalPropertiesExt;
use crate::people::ContextPeopleExt;
use clap::{Parser, Subcommand};
use serde::{Deserialize, Serialize};

pub(crate) trait ExtApi {
    type Args;
    type Retval;

    fn run(context: &mut Context, args: &Self::Args) -> Result<Self::Retval, IxaError>;
}

#[derive(Serialize, Deserialize)]
pub(crate) struct EmptyArgs {}

pub(crate) fn run_ext_api<T: ExtApi>(
    context: &mut Context,
    args: &T::Args,
) -> Result<T::Retval, IxaError> {
    T::run(context, args)
}

pub(crate) struct PopulationExtApi {}
#[derive(Parser, Debug, Deserialize)]
pub(crate) enum PopulationExtApiArgs {
    /// Get the total number of people
    Population,
}

#[derive(Serialize)]
pub(crate) struct PopulationExtApiRetval {
    pub population: usize,
}
impl ExtApi for PopulationExtApi {
    type Args = EmptyArgs;
    type Retval = PopulationExtApiRetval;

    fn run(context: &mut Context, _args: &EmptyArgs) -> Result<PopulationExtApiRetval, IxaError> {
        Ok(PopulationExtApiRetval {
            population: context.get_current_population(),
        })
    }
}

pub(crate) struct GlobalPropertyExtApi {}
#[derive(Serialize, Deserialize, Debug)]
pub(crate) enum GlobalPropertyExtApiRetval {
    List(Vec<String>),
    Value(String),
}
#[derive(Subcommand, Clone, Debug, Serialize, Deserialize)]
/// Access global properties
pub(crate) enum GlobalPropertyExtApiArgsEnum {
    /// List all global properties
    List,

    /// Get the value of a global property
    Get {
        /// The property name
        property: String,
    },
}
#[derive(Parser, Debug, Serialize, Deserialize)]

pub(crate) enum GlobalPropertyExtApiArgs {
    #[command(subcommand)]
    Global(GlobalPropertyExtApiArgsEnum),
}
impl ExtApi for GlobalPropertyExtApi {
    type Args = GlobalPropertyExtApiArgs;
    type Retval = GlobalPropertyExtApiRetval;

    fn run(
        context: &mut Context,
        args: &GlobalPropertyExtApiArgs,
    ) -> Result<GlobalPropertyExtApiRetval, IxaError> {
        let GlobalPropertyExtApiArgs::Global(global_args) = args;

        match global_args {
            GlobalPropertyExtApiArgsEnum::List => Ok(GlobalPropertyExtApiRetval::List(
                context.list_registered_global_properties(),
            )),
            GlobalPropertyExtApiArgsEnum::Get { property: name } => {
                let output = context.get_serialized_value_by_string(name)?;
                match output {
                    Some(value) => Ok(GlobalPropertyExtApiRetval::Value(value)),
                    None => Err(IxaError::IxaError(format!("Property {name} is not set"))),
                }
            }
        }
    }
}

#[derive(Parser, Debug, Deserialize)]
pub(crate) enum NextExtApiArgs {
    /// Continue until the given time and then pause again
    Next {
        /// The time to pause at
        next_time: f64,
    },
}
#[derive(Serialize)]
pub(crate) struct NextExtApiRetval {}
pub(crate) struct NextCommandExtApi {}
impl ExtApi for NextCommandExtApi {
    type Args = NextExtApiArgs;
    type Retval = NextExtApiRetval;

    fn run(context: &mut Context, args: &NextExtApiArgs) -> Result<NextExtApiRetval, IxaError> {
        let NextExtApiArgs::Next { next_time } = args;
        context.schedule_debugger(*next_time);
        Ok(NextExtApiRetval {})
    }
}
