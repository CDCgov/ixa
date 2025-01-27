use crate::context::Context;
use crate::debugger::ContextDebugExt;
use crate::error::IxaError;
use crate::global_properties::ContextGlobalPropertiesExt;

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

pub(crate) mod population {
    use crate::context::Context;
    use crate::external_api::EmptyArgs;
    use crate::people::ContextPeopleExt;
    use crate::IxaError;
    use clap::Parser;
    use serde::{Deserialize, Serialize};

    pub(crate) struct Api {}
    #[derive(Parser, Debug, Deserialize)]
    pub(crate) enum Args {
        /// Get the total number of people
        Population,
    }

    #[derive(Serialize)]
    pub(crate) struct Retval {
        pub population: usize,
    }
    impl super::ExtApi for Api {
        type Args = super::EmptyArgs;
        type Retval = Retval;

        fn run(context: &mut Context, _args: &EmptyArgs) -> Result<Retval, IxaError> {
            Ok(Retval {
                population: context.get_current_population(),
            })
        }
    }
}

pub(crate) mod global_properties {
    use crate::context::Context;
    use crate::global_properties::ContextGlobalPropertiesExt;
    use crate::IxaError;
    use clap::{Parser, Subcommand};
    use serde::{Deserialize, Serialize};

    pub(crate) struct Api {}
    #[derive(Serialize, Deserialize, Debug)]
    pub(crate) enum Retval {
        List(Vec<String>),
        Value(String),
    }
    #[derive(Subcommand, Clone, Debug, Serialize, Deserialize)]
    /// Access global properties
    pub(crate) enum ArgsEnum {
        /// List all global properties
        List,

        /// Get the value of a global property
        Get {
            /// The property name
            property: String,
        },
    }

    #[derive(Parser, Debug, Serialize, Deserialize)]
    pub(crate) enum Args {
        #[command(subcommand)]
        Global(ArgsEnum),
    }
    impl super::ExtApi for Api {
        type Args = Args;
        type Retval = Retval;

        fn run(context: &mut Context, args: &Args) -> Result<Retval, IxaError> {
            let Args::Global(global_args) = args;

            match global_args {
                ArgsEnum::List => Ok(Retval::List(context.list_registered_global_properties())),
                ArgsEnum::Get { property: name } => {
                    let output = context.get_serialized_value_by_string(name)?;
                    match output {
                        Some(value) => Ok(Retval::Value(value)),
                        None => Err(IxaError::IxaError(format!("Property {name} is not set"))),
                    }
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
