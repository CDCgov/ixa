use crate::context::run_with_plugin;
use crate::define_data_plugin;
use crate::external_api::{global_properties, next, people, population, run_ext_api, EmptyArgs};
use crate::Context;
use crate::IxaError;
use clap::{ArgMatches, Command, FromArgMatches, Parser, Subcommand};
use rustyline;

use log::trace;
use std::collections::HashMap;
use std::io::Write;

trait DebuggerCommand {
    /// Handle the command and any inputs; returning true will exit the debugger
    fn handle(
        &self,
        context: &mut Context,
        matches: &ArgMatches,
    ) -> Result<(bool, Option<String>), String>;
    fn extend(&self, command: Command) -> Command;
}

struct Debugger {
    rl: rustyline::DefaultEditor,
    cli: Command,
    commands: HashMap<&'static str, Box<dyn DebuggerCommand>>,
}
define_data_plugin!(DebuggerPlugin, Option<Debugger>, None);

impl Debugger {
    fn get_command(&self, name: &str) -> Option<&dyn DebuggerCommand> {
        self.commands.get(name).map(|command| &**command)
    }

    fn process_command(
        &self,
        l: &str,
        context: &mut Context,
    ) -> Result<(bool, Option<String>), String> {
        let args = shlex::split(l).ok_or("Error splitting lines")?;
        let matches = self
            .cli
            .clone() // cli can only be used once.
            .try_get_matches_from(args)
            .map_err(|e| e.to_string())?;

        if let Some((command, _)) = matches.subcommand() {
            // If the provided command is known, run its handler

            if let Some(handler) = self.get_command(command) {
                return handler.handle(context, &matches);
            }
            // Unexpected command: print an error
            return Err(format!("error: Unknown command: {command}"));
        }

        unreachable!("subcommand required");
    }
}

struct PopulationCommand;
impl DebuggerCommand for PopulationCommand {
    fn handle(
        &self,
        context: &mut Context,
        _matches: &ArgMatches,
    ) -> Result<(bool, Option<String>), String> {
        let output = format!(
            "{}",
            run_ext_api::<population::Api>(context, &EmptyArgs {})
                .unwrap()
                .population
        );
        Ok((false, Some(output)))
    }
    fn extend(&self, command: Command) -> Command {
        population::Args::augment_subcommands(command)
    }
}

struct PeopleCommand;
impl DebuggerCommand for PeopleCommand {
    fn extend(&self, command: Command) -> Command {
        people::Args::augment_subcommands(command)
    }
    fn handle(
        &self,
        context: &mut Context,
        matches: &ArgMatches,
    ) -> Result<(bool, Option<String>), String> {
        let args = people::Args::from_arg_matches(matches).unwrap();
        match run_ext_api::<people::Api>(context, &args) {
            Ok(people::Retval::Properties(props)) => Ok((
                false,
                Some(
                    props
                        .into_iter()
                        .map(|(k, v)| format!("{k}: {v}"))
                        .collect::<Vec<_>>()
                        .join("\n"),
                ),
            )),
            Ok(people::Retval::PropertyNames(names)) => Ok((
                false,
                Some(format!("Available properties:\n{}", names.join("\n"))),
            )),
            Ok(people::Retval::Tabulated(rows)) => Ok((
                false,
                Some(
                    rows.into_iter()
                        .map(|(props, count)| {
                            format!(
                                "{}: {}",
                                count,
                                props
                                    .into_iter()
                                    .map(|(k, v)| format!("{k}={v}"))
                                    .collect::<Vec<_>>()
                                    .join(", ")
                            )
                        })
                        .collect::<Vec<_>>()
                        .join("\n"),
                ),
            )),
            Err(e) => Ok((false, Some(format!("error: {e}")))),
        }
    }
}

struct GlobalPropertyCommand;
impl DebuggerCommand for GlobalPropertyCommand {
    fn extend(&self, command: Command) -> Command {
        global_properties::Args::augment_subcommands(command)
    }
    fn handle(
        &self,
        context: &mut Context,
        matches: &ArgMatches,
    ) -> Result<(bool, Option<String>), String> {
        let args = global_properties::Args::from_arg_matches(matches).unwrap();
        let ret = run_ext_api::<global_properties::Api>(context, &args);
        match ret {
            Err(IxaError::IxaError(e)) => Ok((false, Some(format!("error: {e}")))),
            Err(e) => Ok((false, Some(format!("error: {e}")))),
            Ok(global_properties::Retval::List(properties)) => Ok((
                false,
                Some(format!(
                    "{} global properties registered:\n{}",
                    properties.len(),
                    properties.join("\n")
                )),
            )),
            Ok(global_properties::Retval::Value(value)) => Ok((false, Some(value))),
        }
    }
}

struct NextCommand;
/// Adds a new debugger breakpoint at t
impl DebuggerCommand for NextCommand {
    fn handle(
        &self,
        context: &mut Context,
        matches: &ArgMatches,
    ) -> Result<(bool, Option<String>), String> {
        let args = next::Args::from_arg_matches(matches).unwrap();
        match run_ext_api::<next::Api>(context, &args) {
            Err(IxaError::IxaError(e)) => Ok((false, Some(format!("error: {e}")))),
            Ok(_) => {
                let next::Args::Next { next_time } = args;
                context.schedule_debugger(next_time);
                Ok((true, None))
            }
            _ => unimplemented!(),
        }
    }
    fn extend(&self, command: Command) -> Command {
        next::Args::augment_subcommands(command)
    }
}

struct ContinueCommand;
#[derive(Parser, Debug)]
enum ContinueSubcommand {
    /// Exits the debugger and continues the simulation
    Continue,
}
impl DebuggerCommand for ContinueCommand {
    fn handle(
        &self,
        _context: &mut Context,
        _matches: &ArgMatches,
    ) -> Result<(bool, Option<String>), String> {
        Ok((true, None))
    }
    fn extend(&self, command: Command) -> Command {
        ContinueSubcommand::augment_subcommands(command)
    }
}

// Build the debugger context.
fn init(context: &mut Context) {
    let debugger = context.get_data_container_mut(DebuggerPlugin);

    if debugger.is_none() {
        let mut commands: HashMap<&'static str, Box<dyn DebuggerCommand>> = HashMap::new();
        commands.insert("people", Box::new(PeopleCommand));
        commands.insert("population", Box::new(PopulationCommand));
        commands.insert("next", Box::new(NextCommand));
        commands.insert("continue", Box::new(ContinueCommand));
        commands.insert("global", Box::new(GlobalPropertyCommand));

        let mut cli = Command::new("repl")
            .multicall(true)
            .arg_required_else_help(true)
            .subcommand_required(true)
            .subcommand_value_name("DEBUGGER")
            .subcommand_help_heading("IXA DEBUGGER")
            .help_template("{all-args}");

        for handler in commands.values() {
            cli = handler.extend(cli);
        }

        *debugger = Some(Debugger {
            rl: rustyline::DefaultEditor::new().unwrap(),
            cli,
            commands,
        });
    }
}

fn exit_debugger() -> ! {
    println!("Got Ctrl-D, Exiting...");
    std::process::exit(0);
}

/// Starts the debugger and pauses execution
fn start_debugger(context: &mut Context, debugger: &mut Debugger) -> Result<(), IxaError> {
    init(context);
    let t = context.get_current_time();

    println!("Debugging simulation at t={t}");
    loop {
        let line = match debugger.rl.readline(&format!("t={t} $ ")) {
            Ok(line) => line,
            Err(
                rustyline::error::ReadlineError::WindowResized
                | rustyline::error::ReadlineError::Interrupted,
            ) => continue,
            Err(rustyline::error::ReadlineError::Eof) => exit_debugger(),
            Err(err) => return Err(IxaError::IxaError(format!("Read error: {err}"))),
        };
        debugger
            .rl
            .add_history_entry(line.clone())
            .expect("Should be able to add to input");
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        match debugger.process_command(line, context) {
            Ok((quit, message)) => {
                if quit {
                    break;
                }
                if let Some(message) = message {
                    let _ = writeln!(std::io::stdout(), "{message}");
                    std::io::stdout().flush().unwrap();
                }
            }
            Err(err) => {
                write!(std::io::stdout(), "{err}").map_err(|e| e.to_string())?;
                std::io::stdout().flush().unwrap();
            }
        }
    }

    Ok(())
}

pub trait ContextDebugExt {
    /// Schedule the simulation to pause at time t and start the debugger.
    /// This will give you a REPL which allows you to inspect the state of
    /// the simulation (type help to see a list of commands)
    ///
    /// # Errors
    /// Internal debugger errors e.g., reading or writing to stdin/stdout;
    /// errors in Ixa are printed to stdout
    fn schedule_debugger(&mut self, t: f64);
}

impl ContextDebugExt for Context {
    fn schedule_debugger(&mut self, t: f64) {
        trace!("scheduling debugger");
        self.add_plan(t, |context| {
            init(context);
            run_with_plugin::<DebuggerPlugin>(context, |context, data_container| {
                start_debugger(context, data_container.as_mut().unwrap())
                    .expect("Error in debugger");
            });
        });
    }
}

#[cfg(test)]
mod tests {
    use super::{init, run_with_plugin, DebuggerPlugin};
    use crate::tests::run_external_runner;
    use crate::{define_global_property, define_person_property, ContextGlobalPropertiesExt};
    use crate::{Context, ContextPeopleExt};

    fn process_line(line: &str, context: &mut Context) -> (bool, Option<String>) {
        // Temporarily take the data container out of context so that
        // we can operate on context.
        init(context);
        let data_container = context.get_data_container_mut(DebuggerPlugin);
        let debugger = data_container.take().unwrap();

        let res = debugger.process_command(line, context).unwrap();
        let data_container = context.get_data_container_mut(DebuggerPlugin);
        *data_container = Some(debugger);
        res
    }

    define_global_property!(FooGlobal, String);
    define_global_property!(BarGlobal, u32);
    define_person_property!(Age, u8);
    define_person_property!(Smile, u32);

    #[test]
    fn test_cli_debugger_integration() {
        run_external_runner("runner_test_debug")
            .unwrap()
            .args(["--debugger", "1.0"])
            .write_stdin("population\n")
            .write_stdin("continue\n")
            .assert()
            .success();
    }

    #[test]
    fn test_cli_debugger_population() {
        let context = &mut Context::new();
        // Add 2 people
        context.add_person(()).unwrap();
        context.add_person(()).unwrap();

        let (quits, output) = process_line("population\n", context);

        assert!(!quits, "should not exit");
        assert!(output.unwrap().contains('2'));
    }

    #[test]
    fn test_cli_debugger_people_get() {
        let context = &mut Context::new();
        // Add 2 people
        context.add_person((Age, 10)).unwrap();
        context.add_person((Age, 5)).unwrap();
        assert_eq!(context.remaining_plan_count(), 0);
        let (_, output) = process_line("people get 0 Age", context);
        assert_eq!(output.unwrap(), "Age: 10");
        let (_, output) = process_line("people get 1 Age", context);
        assert_eq!(output.unwrap(), "Age: 5");
    }

    #[test]
    fn test_cli_debugger_people_properties() {
        let context = &mut Context::new();
        // Add 2 people
        context.add_person(((Age, 10), (Smile, 50))).unwrap();
        context.add_person(((Age, 5), (Smile, 60))).unwrap();
        let (_, output) = process_line("people get 0 Smile", context);
        assert_eq!(output.unwrap(), "Smile: 50");
        let (_, output) = process_line("people properties", context);
        let properties = output.unwrap();
        assert!(properties.contains("Smile"));
        assert!(properties.contains("Age"));
    }

    #[test]
    fn test_cli_debugger_people_tabulate() {
        let context = &mut Context::new();
        // Add 3 people
        context.add_person(((Age, 10), (Smile, 50))).unwrap();
        context.add_person(((Age, 10), (Smile, 60))).unwrap();
        context.add_person(((Age, 10), (Smile, 60))).unwrap();
        let (_, output) = process_line("people tabulate Age", context);
        assert_eq!(output.unwrap(), "3: Age=10");
        let (_, output) = process_line("people tabulate Smile", context);
        let results = output.unwrap();
        assert!(results.contains("1: Smile=50"));
        assert!(results.contains("2: Smile=60"));
    }

    #[test]
    fn test_cli_debugger_global_list() {
        let context = &mut Context::new();
        let (_quits, output) = process_line("global list\n", context);
        let expected = format!(
            "{} global properties registered:",
            context.list_registered_global_properties().len()
        );
        // Note: the global property names are also listed as part of the output
        assert!(output.unwrap().contains(&expected));
    }

    #[test]
    fn test_cli_debugger_global_no_args() {
        let input = "global get\n";
        let context = &mut Context::new();
        init(context);
        // We can't use process_line here because we an expect an error to be
        // returned rather than string output
        run_with_plugin::<DebuggerPlugin>(context, |context, data_container| {
            let debugger = data_container.take().unwrap();

            let result = debugger.process_command(input, context);
            let data_container = context.get_data_container_mut(DebuggerPlugin);
            *data_container = Some(debugger);

            assert!(result.is_err());
            assert!(result
                .unwrap_err()
                .contains("required arguments were not provided"));
        });
    }

    #[test]
    fn test_cli_debugger_global_get_unregistered_prop() {
        let context = &mut Context::new();
        let (_quits, output) = process_line("global get NotExist\n", context);
        assert_eq!(output.unwrap(), "error: No global property: NotExist");
    }

    #[test]
    fn test_cli_debugger_global_get_registered_prop() {
        let context = &mut Context::new();
        context
            .set_global_property_value(FooGlobal, "hello".to_string())
            .unwrap();
        let (_quits, output) = process_line("global get ixa.FooGlobal\n", context);
        assert_eq!(output.unwrap(), "\"hello\"");
    }

    #[test]
    fn test_cli_debugger_global_get_empty_prop() {
        define_global_property!(EmptyGlobal, String);
        let context = &mut Context::new();
        let (_quits, output) = process_line("global get ixa.EmptyGlobal\n", context);
        assert_eq!(
            output.unwrap(),
            "error: Property ixa.EmptyGlobal is not set"
        );
    }

    #[test]
    fn test_cli_continue() {
        let context = &mut Context::new();
        let (quits, _) = process_line("continue\n", context);
        assert!(quits, "should exit");
    }

    #[test]
    fn test_cli_next() {
        let context = &mut Context::new();
        assert_eq!(context.remaining_plan_count(), 0);
        let (quits, _) = process_line("next 2\n", context);
        assert!(quits, "should exit");
        assert_eq!(
            context.remaining_plan_count(),
            1,
            "should schedule a plan for the debugger to pause"
        );
    }
}
