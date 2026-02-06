// Now all features of the external API are used internally, so we expect dead code.
#![allow(dead_code)]

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

use crate::context::Context;
use crate::error::IxaError;

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

pub(crate) mod global_properties {
    use clap::{Parser, Subcommand};
    use serde::{Deserialize, Serialize};

    use crate::context::Context;
    use crate::global_properties::ContextGlobalPropertiesExt;
    use crate::IxaError;

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

pub(crate) mod breakpoint {
    use clap::{Parser, Subcommand};
    use serde::{Deserialize, Serialize};

    use crate::context::Context;
    use crate::debugger::enter_debugger;
    use crate::{trace, IxaError};

    #[derive(Subcommand, Clone, Debug, Serialize, Deserialize)]
    /// Manipulate Debugger Breakpoints
    pub(crate) enum ArgsEnum {
        /// List all scheduled breakpoints
        List,
        /// Set a breakpoint at a given time
        Set {
            #[arg(required = true)]
            time: f64,
            #[arg(long, hide = true, default_value_t = true)]
            console: bool,
        },
        /// Delete the breakpoint with the specified id.
        /// Providing the `--all` option removes all breakpoints.
        #[group(multiple = false, required = true)]
        Delete {
            /// The ID of the breakpoint to delete
            #[arg(value_name = "ID")]
            id: Option<u32>,

            /// Remove all breakpoints
            #[arg(long, action)]
            all: bool,
        },
        /// Disables but does not delete breakpoints globally
        Disable,
        /// Enables breakpoints globally
        Enable,
    }

    #[derive(Parser, Debug, Serialize, Deserialize)]
    pub(crate) enum Args {
        #[command(subcommand)]
        Breakpoint(ArgsEnum),
    }

    #[derive(Serialize, Deserialize, Debug)]
    pub(crate) enum Retval {
        List(Vec<String>),
        Ok,
    }

    pub(crate) struct Api {}
    impl super::ExtApi for Api {
        type Args = Args;
        type Retval = Retval;

        fn run(context: &mut Context, args: &Args) -> Result<Retval, IxaError> {
            let Args::Breakpoint(breakpoint_args) = args;

            match breakpoint_args {
                ArgsEnum::List => {
                    trace!("Listing breakpoints");
                    let list = context.list_breakpoints(0);
                    let list = list
                        .iter()
                        .map(|schedule| {
                            format!(
                                "{}: t={} ({})",
                                schedule.plan_id, schedule.time, schedule.priority
                            )
                        })
                        .collect::<Vec<String>>();
                    Ok(Retval::List(list))
                }

                #[allow(unused_variables)]
                ArgsEnum::Set { time, console } => {
                    if *time < context.get_current_time() {
                        return Err(IxaError::from(format!(
                            "Breakpoint time {time} is in the past"
                        )));
                    }
                    context.schedule_debugger(*time, None, Box::new(enter_debugger));

                    trace!("Breakpoint set at t={time}");
                    Ok(Retval::Ok)
                }

                ArgsEnum::Delete { id, all } => {
                    if let Some(id) = id {
                        assert!(!all);
                        trace!("Deleting breakpoint {id}");
                        let cancelled = context.delete_breakpoint(u64::from(*id));
                        if cancelled.is_none() {
                            Err(IxaError::from(format!(
                                "Attempted to delete a nonexistent breakpoint {id}",
                            )))
                        } else {
                            Ok(Retval::Ok)
                        }
                    } else {
                        assert!(all);
                        trace!("Deleting all breakpoints");
                        context.clear_breakpoints();
                        Ok(Retval::Ok)
                    }
                }

                ArgsEnum::Disable => {
                    trace!("Disabling all breakpoints");
                    context.disable_breakpoints();
                    Ok(Retval::Ok)
                }

                ArgsEnum::Enable => {
                    trace!("Enabling all breakpoints");
                    context.enable_breakpoints();
                    Ok(Retval::Ok)
                }
            }
        }
    }
}

pub(crate) mod next {
    use clap::Parser;
    use serde::Serialize;
    use serde_derive::Deserialize;

    use crate::context::Context;
    use crate::external_api::EmptyArgs;
    use crate::IxaError;

    #[derive(Parser, Debug, Serialize, Deserialize)]
    pub enum Args {
        /// Execute the next item in the event loop
        Next,
    }

    #[derive(Serialize)]
    pub(crate) enum Retval {
        Ok,
    }
    #[allow(unused)]
    pub(crate) struct Api {}
    impl super::ExtApi for Api {
        type Args = EmptyArgs;
        type Retval = Retval;

        fn run(_context: &mut Context, _args: &EmptyArgs) -> Result<Retval, IxaError> {
            // This is a no-op which allows for arg checking.
            Ok(Retval::Ok)
        }
    }
}

pub(crate) mod halt {
    use clap::Parser;
    use serde::Serialize;
    use serde_derive::Deserialize;

    use crate::context::Context;
    use crate::external_api::EmptyArgs;
    use crate::IxaError;

    #[derive(Parser, Debug, Serialize, Deserialize)]
    pub enum Args {
        /// End the simulation
        Halt,
    }

    #[derive(Serialize)]
    pub(crate) enum Retval {
        Ok,
    }
    #[allow(unused)]
    pub(crate) struct Api {}
    impl super::ExtApi for Api {
        type Args = EmptyArgs;
        type Retval = Retval;

        fn run(_context: &mut Context, _args: &EmptyArgs) -> Result<Retval, IxaError> {
            // This is a no-op which allows for arg checking.
            Ok(Retval::Ok)
        }
    }
}

pub(crate) mod r#continue {
    use clap::Parser;
    use serde_derive::{Deserialize, Serialize};

    use crate::context::Context;
    use crate::external_api::EmptyArgs;
    use crate::IxaError;

    #[derive(Parser, Debug, Serialize, Deserialize)]
    pub enum Args {
        /// Continue running the simulation
        Continue,
    }

    #[derive(Serialize)]
    pub(crate) enum Retval {
        Ok,
    }
    #[allow(unused)]
    pub(crate) struct Api {}
    impl super::ExtApi for Api {
        type Args = EmptyArgs;
        type Retval = Retval;

        fn run(_context: &mut Context, _args: &EmptyArgs) -> Result<Retval, IxaError> {
            // This is a no-op which allows for arg checking.
            Ok(Retval::Ok)
        }
    }
}

pub(crate) mod time {
    #![allow(clippy::float_cmp)]
    use clap::Parser;
    use serde::{Deserialize, Serialize};

    use crate::context::Context;
    use crate::external_api::EmptyArgs;
    use crate::IxaError;

    pub(crate) struct Api {}
    #[derive(Parser, Debug, Deserialize)]
    pub(crate) enum Args {
        /// Get the time of the simulation.
        Time,
    }

    #[derive(Serialize)]
    pub(crate) struct Retval {
        pub time: f64,
    }
    impl super::ExtApi for Api {
        type Args = super::EmptyArgs;
        type Retval = Retval;

        fn run(context: &mut Context, _args: &EmptyArgs) -> Result<Retval, IxaError> {
            Ok(Retval {
                time: context.get_current_time(),
            })
        }
    }

    #[cfg(test)]
    mod test {
        use crate::Context;

        #[test]
        fn test() {
            let mut context = Context::new();

            let result = crate::external_api::run_ext_api::<super::Api>(
                &mut context,
                &crate::external_api::EmptyArgs {},
            );

            assert_eq!(result.unwrap().time, 0.0);
        }
    }
}
