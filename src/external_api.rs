// Now all features of the external API are used internally, so we expect dead code.
#![allow(dead_code)]

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
        type Args = EmptyArgs;
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

pub(crate) mod breakpoint {
    use crate::context::Context;
    use crate::debugger::enter_debugger;
    #[cfg(feature = "web_api")]
    use crate::web_api::handle_web_api_with_plugin;
    use crate::{trace, IxaError};
    use clap::{Parser, Subcommand};
    use serde::{Deserialize, Serialize};

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

                    #[cfg(feature = "web_api")]
                    if *console {
                        context.schedule_debugger(*time, None, Box::new(enter_debugger));
                    } else {
                        context.schedule_debugger(
                            *time,
                            None,
                            Box::new(handle_web_api_with_plugin),
                        );
                    }
                    #[cfg(not(feature = "web_api"))]
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
    use crate::context::Context;
    use crate::external_api::EmptyArgs;
    use crate::IxaError;
    use clap::Parser;
    use serde::Serialize;
    use serde_derive::Deserialize;

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
    use crate::context::Context;
    use crate::external_api::EmptyArgs;
    use crate::IxaError;
    use clap::Parser;
    use serde::Serialize;
    use serde_derive::Deserialize;

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
    use crate::context::Context;
    use crate::external_api::EmptyArgs;
    use crate::IxaError;
    use clap::Parser;
    use serde_derive::{Deserialize, Serialize};

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

pub(crate) mod people {
    use crate::{HashMap, HashMapExt};
    use std::cell::RefCell;

    use crate::people::{external_api::ContextPeopleExtCrate, ContextPeopleExt, PersonId};
    use crate::Context;
    use crate::IxaError;
    use clap::{Parser, Subcommand};
    use serde::{Deserialize, Serialize};

    fn person_id_from_str(s: &str) -> Result<PersonId, String> {
        match s.parse::<usize>() {
            Ok(id) => Ok(PersonId(id)),
            Err(_) => Err("Person id must be an integer".to_string()),
        }
    }

    #[derive(Subcommand, Clone, Debug, Serialize, Deserialize)]
    pub(crate) enum ArgsEnum {
        /// Get the value of a property for a person
        Get {
            #[arg(value_parser = person_id_from_str)]
            person_id: PersonId,
            property: String,
        },
        Query {
            #[clap(skip)]
            properties: Vec<(String, String)>,
        },
        /// Tabulate the values of a set of properties
        Tabulate {
            properties: Vec<String>,
        },
        Properties,
    }

    #[derive(Parser, Debug, Serialize, Deserialize)]
    pub(crate) enum Args {
        /// Access people properties
        #[command(subcommand)]
        People(ArgsEnum),
    }

    #[derive(Serialize, Debug, Eq, PartialEq)]
    pub(crate) enum Retval {
        Properties(Vec<(String, String)>),
        Tabulated(Vec<(HashMap<String, String>, usize)>),
        PropertyNames(Vec<String>),
    }
    pub(crate) struct Api {}

    impl super::ExtApi for Api {
        type Args = Args;
        type Retval = Retval;

        fn run(context: &mut Context, args: &Args) -> Result<Retval, IxaError> {
            match args {
                Args::People(args_enum) => match args_enum {
                    ArgsEnum::Get {
                        person_id,
                        property,
                    } => {
                        if person_id.0 >= context.get_current_population() {
                            return Err(IxaError::IxaError(format!(
                                "No person with id {person_id:?}"
                            )));
                        }
                        let value = context.get_person_property_by_name(property, *person_id)?;
                        Ok(Retval::Properties(vec![(property.clone(), value)]))
                    }
                    ArgsEnum::Query { properties: _ } => Err(IxaError::IxaError(String::from(
                        "People querying not implemented",
                    ))),
                    ArgsEnum::Tabulate { properties } => {
                        let results = RefCell::new(Vec::new());

                        context.tabulate_person_properties_by_name(
                            properties.clone(),
                            |_, values, count| {
                                let mut hm = HashMap::new();
                                for (key, value) in properties.iter().zip(values.iter()) {
                                    hm.insert(key.clone(), value.clone());
                                }
                                results.borrow_mut().push((hm, count));
                            },
                        )?;
                        Ok(Retval::Tabulated(results.take()))
                    }
                    ArgsEnum::Properties => {
                        Ok(Retval::PropertyNames(context.get_person_property_names()))
                    }
                },
            }
        }
    }

    #[cfg(test)]
    mod test {
        use super::*;
        use crate::external_api::run_ext_api;
        use crate::{define_person_property, Context};
        use crate::{HashSet, HashSetExt};
        #[test]
        fn query_nonexistent_user() {
            let mut context = Context::new();

            let res = run_ext_api::<super::Api>(
                &mut context,
                &Args::People(ArgsEnum::Get {
                    person_id: PersonId(0),
                    property: String::from("abc"),
                }),
            );

            println!("{res:?}");
            assert!(matches!(res, Err(IxaError::IxaError(_))));
        }

        #[test]
        fn query_nonexistent_property() {
            let mut context = Context::new();
            let _ = context.add_person(());
            let res = run_ext_api::<super::Api>(
                &mut context,
                &Args::People(ArgsEnum::Get {
                    person_id: PersonId(0),
                    property: String::from("abc"),
                }),
            );

            println!("{res:?}");
            assert!(matches!(res, Err(IxaError::IxaError(_))));
        }

        define_person_property!(Age, u8);

        #[test]
        fn query_valid_property() {
            let mut context = Context::new();
            let _ = context.add_person((Age, 10));
            let res = run_ext_api::<super::Api>(
                &mut context,
                &Args::People(ArgsEnum::Get {
                    person_id: PersonId(0),
                    property: String::from("Age"),
                }),
            );

            println!("{res:?}");
            let res = res.unwrap();
            #[allow(clippy::match_wildcard_for_single_variants)]
            match res {
                Retval::Properties(val) => {
                    assert_eq!(val, vec![(String::from("Age"), String::from("10"))]);
                }
                _ => panic!("Unexpected result"),
            }
        }

        #[test]
        fn tabulate() {
            let mut context = Context::new();
            let _ = context.add_person((Age, 10));
            let _ = context.add_person((Age, 20));

            let res = run_ext_api::<super::Api>(
                &mut context,
                &Args::People(ArgsEnum::Tabulate {
                    properties: vec![String::from("Age")],
                }),
            );
            println!("{res:?}");
            let res = res.unwrap();
            let mut expected = HashSet::new();
            expected.insert(String::from("10"));
            expected.insert(String::from("20"));

            #[allow(clippy::match_wildcard_for_single_variants)]
            match res {
                Retval::Tabulated(val) => {
                    for (columns, ct) in val {
                        assert_eq!(ct, 1);
                        let age = columns.get("Age").unwrap();
                        assert!(expected.remove(age));
                    }
                    assert_eq!(expected.len(), 0);
                }
                _ => panic!("Unexpected result"),
            }
        }

        #[test]
        fn get_person_property_names() {
            let mut context = Context::new();
            let _ = context.add_person((Age, 10));
            let _ = context.add_person((Age, 20));

            let res = run_ext_api::<super::Api>(&mut context, &Args::People(ArgsEnum::Properties));
            println!("{res:?}");
            let res = res.unwrap();

            #[allow(clippy::match_wildcard_for_single_variants)]
            match res {
                Retval::PropertyNames(names) => assert_eq!(names, vec!["Age"]),
                _ => panic!("Unexpected result"),
            }
        }
    }
}

pub(crate) mod time {
    #![allow(clippy::float_cmp)]
    use crate::context::Context;
    use crate::external_api::EmptyArgs;
    use crate::IxaError;
    use clap::Parser;
    use serde::{Deserialize, Serialize};

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
