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

pub(crate) mod people {
    use std::cell::RefCell;
    use std::collections::HashMap;

    use crate::people::{external_api::ContextPeopleExtCrate, ContextPeopleExt, PersonId};
    use crate::Context;
    use crate::IxaError;
    use serde::{Deserialize, Serialize};

    #[derive(Deserialize)]
    pub(crate) enum ArgsEnum {
        Get {
            person_id: PersonId,
            property: String,
        },
        Query {
            properties: Vec<(String, String)>,
        },
        Tabulate {
            properties: Vec<String>,
        },
        Properties,
    }

    #[derive(Deserialize)]
    pub(crate) enum Args {
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
            let Args::People(args) = args;

            match args {
                ArgsEnum::Get {
                    person_id,
                    property,
                } => {
                    if person_id.0 >= context.get_current_population() {
                        return Err(IxaError::IxaError(format!("No person with id {person_id}")));
                    }

                    let value = context.get_person_property_by_name(property, *person_id)?;
                    Ok(Retval::Properties(vec![(property.to_string(), value)]))
                }
                ArgsEnum::Query {
                    properties: _properties,
                } => Err(IxaError::IxaError(String::from(
                    "People querying not implemented",
                ))),
                ArgsEnum::Tabulate { properties } => {
                    let results: RefCell<Vec<(HashMap<String, String>, usize)>> =
                        RefCell::new(Vec::new());

                    context.tabulate_person_properties_by_name(
                        properties.clone(),
                        |_context, values, count| {
                            let mut hm = HashMap::new();
                            for i in 0..properties.len() {
                                hm.insert(properties[i].clone(), values[i].clone());
                            }
                            results.borrow_mut().push((hm, count));
                        },
                    )?;
                    Ok(Retval::Tabulated(results.take()))
                }
                ArgsEnum::Properties => {
                    Ok(Retval::PropertyNames(context.get_person_property_names()))
                }
            }
        }
    }

    #[cfg(test)]
    mod test {
        use super::*;
        use crate::external_api::run_ext_api;
        use crate::{define_person_property, Context};
        use std::collections::HashSet;
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

            assert_eq!(result.time, 0.0);
        }
    }
}
