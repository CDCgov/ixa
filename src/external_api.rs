use crate::context::Context;
use crate::error::IxaError;
use serde::{de::DeserializeOwned, Deserialize, Serialize};

pub(crate) trait ExtApi {
    type Args: DeserializeOwned;
    type Retval: Serialize;

    fn run(context: &mut Context, args: &Self::Args) -> Result<Self::Retval, IxaError>;
}

#[derive(Serialize, Deserialize, Debug)]
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

pub(crate) mod next {
    use crate::context::Context;
    use crate::IxaError;
    use clap::Parser;
    use serde::{Deserialize, Serialize};

    #[derive(Parser, Debug, Deserialize)]
    pub(crate) enum Args {
        /// Continue until the given time and then pause again
        Next {
            /// The time to pause at
            next_time: f64,
        },
    }
    #[derive(Serialize)]
    pub(crate) struct Retval {}
    pub(crate) struct Api {}
    impl super::ExtApi for Api {
        type Args = Args;
        type Retval = Retval;

        fn run(context: &mut Context, args: &Args) -> Result<Retval, IxaError> {
            let Args::Next { next_time } = args;
            if *next_time < context.get_current_time() {
                return Err(IxaError::from(format!(
                    "Breakpoint time {next_time} is in the past"
                )));
            }
            Ok(Retval {})
        }
    }
}

pub(crate) mod r#continue {
    use crate::context::Context;
    use crate::IxaError;
    use clap::Parser;
    use serde::{Deserialize, Serialize};

    #[derive(Parser, Debug, Deserialize)]
    pub(crate) enum Args {}
    #[derive(Serialize)]
    pub(crate) struct Retval {}
    #[allow(unused)]
    pub(crate) struct Api {}
    impl super::ExtApi for Api {
        type Args = Args;
        type Retval = Retval;

        fn run(_context: &mut Context, _args: &Args) -> Result<Retval, IxaError> {
            // This is a no-op which allows for arg checking.
            Ok(Retval {})
        }
    }
}
