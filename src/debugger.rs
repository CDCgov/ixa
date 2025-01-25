use crate::context::run_with_plugin;
use crate::define_data_plugin;
use crate::Context;
use crate::ContextGlobalPropertiesExt;
use crate::ContextPeopleExt;
use crate::IxaError;
use clap::value_parser;
use clap::{Arg, ArgMatches, Command};
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
    fn about(&self) -> &'static str;
    fn extend(&self, subcommand: Command) -> Command {
        subcommand
    }
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

        if let Some((command, sub_matches)) = matches.subcommand() {
            // If the provided command is known, run its handler

            if let Some(handler) = self.get_command(command) {
                return handler.handle(context, sub_matches);
            }
            // Unexpected command: print an error
            return Err(format!("Unknown command: {command}"));
        }

        unreachable!("subcommand required");
    }
}

/// Returns the current population of the simulation
struct PopulationCommand;
impl DebuggerCommand for PopulationCommand {
    fn about(&self) -> &'static str {
        "Get the total number of people"
    }
    fn handle(
        &self,
        context: &mut Context,
        _matches: &ArgMatches,
    ) -> Result<(bool, Option<String>), String> {
        let output = format!("{}", context.get_current_population());
        Ok((false, Some(output)))
    }
}

/// Helper function for displaying properties
fn available_properties_str(context: &Context) -> String {
    let properties = context.list_registered_global_properties();
    format!(
        "{} global properties registered:\n{}",
        properties.len(),
        properties.join("\n")
    )
}

struct GlobalPropertyCommand;
impl DebuggerCommand for GlobalPropertyCommand {
    fn about(&self) -> &'static str {
        "Get the value for a global property"
    }
    fn extend(&self, subcommand: Command) -> Command {
        subcommand
            .subcommand_required(true)
            .subcommand(Command::new("list").about("List all global properties"))
            .subcommand(
                Command::new("get")
                    .about("Get the value of a global property")
                    .arg(
                        Arg::new("property")
                            .help("The name of the global property")
                            .value_parser(value_parser!(String))
                            .required(true),
                    ),
            )
    }
    fn handle(
        &self,
        context: &mut Context,
        matches: &ArgMatches,
    ) -> Result<(bool, Option<String>), String> {
        match matches.subcommand() {
            Some(("list", _)) => Ok((false, Some(available_properties_str(context)))),
            Some(("get", m)) => {
                let name = m.get_one::<String>("property").unwrap();
                let output = context.get_serialized_value_by_string(name);
                if output.is_err() {
                    return Ok((false, output.err().map(|e| e.to_string())));
                }
                match output.unwrap() {
                    Some(value) => Ok((false, Some(value))),
                    None => Ok((false, Some(format!("Property {name} is not set")))),
                }
            }
            // This is required by the compiler will never get hit because
            // .subcommand_required(true) is set in extend
            _ => unimplemented!("subcommand required"),
        }
    }
}

/// Adds a new debugger breakpoint at t
struct NextCommand;
impl DebuggerCommand for NextCommand {
    fn about(&self) -> &'static str {
        "Continue until the given time and then pause again"
    }
    fn extend(&self, subcommand: Command) -> Command {
        subcommand.arg(
            Arg::new("t")
                .help("The next breakpoint (e.g., 4.2)")
                .value_parser(value_parser!(f64))
                .required(true),
        )
    }
    fn handle(
        &self,
        context: &mut Context,
        matches: &ArgMatches,
    ) -> Result<(bool, Option<String>), String> {
        let t = *matches.get_one::<f64>("t").unwrap();
        context.schedule_debugger(t);
        Ok((true, None))
    }
}

/// Exits the debugger and continues the simulation
struct ContinueCommand;
impl DebuggerCommand for ContinueCommand {
    fn about(&self) -> &'static str {
        "Continue the simulation and exit the debugger"
    }
    fn handle(
        &self,
        _context: &mut Context,
        _matches: &ArgMatches,
    ) -> Result<(bool, Option<String>), String> {
        Ok((true, None))
    }
}

// Build the debugger context.
fn init(context: &mut Context) {
    let debugger = context.get_data_container_mut(DebuggerPlugin);

    if debugger.is_none() {
        let mut commands: HashMap<&'static str, Box<dyn DebuggerCommand>> = HashMap::new();
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

        for (name, handler) in &commands {
            let subcommand =
                handler.extend(Command::new(name).about(handler.about()).help_template(
                    "{about-with-newline}\n{usage-heading}\n    {usage}\n\n{all-args}{after-help}",
                ));
            cli = cli.subcommand(subcommand);
        }
        *debugger = Some(Debugger {
            rl: rustyline::DefaultEditor::new().unwrap(),
            cli,
            commands,
        });
    }
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
            Err(rustyline::error::ReadlineError::Eof) => return Ok(()),
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
    use crate::{define_global_property, ContextGlobalPropertiesExt};
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

    #[test]
    fn test_cli_debugger_integration() {
        assert_cmd::Command::cargo_bin("runner_test_debug")
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
        assert_eq!(
            output.unwrap(),
            "Error: IxaError(\"No global property: NotExist\")"
        );
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
        assert_eq!(output.unwrap(), "Property ixa.EmptyGlobal is not set");
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
