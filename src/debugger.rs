use crate::define_data_plugin;
use crate::external_api::{
    breakpoint, global_properties, halt, next, people, population, run_ext_api, EmptyArgs,
};
use crate::{trace, Context, IxaError};
use crate::{HashMap, HashMapExt};
use clap::{ArgMatches, Command, FromArgMatches, Parser, Subcommand};
use rustyline;

use std::fmt::Write;

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
define_data_plugin!(DebuggerPlugin, Option<Debugger>, |_context| {
    // Build the debugger context.
    trace!("initializing debugger");
    let mut commands: HashMap<&'static str, Box<dyn DebuggerCommand>> = HashMap::new();
    commands.insert("breakpoint", Box::new(BreakpointCommand));
    commands.insert("continue", Box::new(ContinueCommand));
    commands.insert("global", Box::new(GlobalPropertyCommand));
    commands.insert("halt", Box::new(HaltCommand));
    commands.insert("next", Box::new(NextCommand));
    commands.insert("people", Box::new(PeopleCommand));
    commands.insert("population", Box::new(PopulationCommand));

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

    Some(Debugger {
        rl: rustyline::DefaultEditor::new().unwrap(),
        cli,
        commands,
    })
});

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

/// Exits the debugger and ends the simulation.
struct HaltCommand;
impl DebuggerCommand for HaltCommand {
    fn handle(
        &self,
        context: &mut Context,
        _matches: &ArgMatches,
    ) -> Result<(bool, Option<String>), String> {
        context.shutdown();
        Ok((true, None))
    }
    fn extend(&self, command: Command) -> Command {
        halt::Args::augment_subcommands(command)
    }
}

/// Adds a new debugger breakpoint at t
struct NextCommand;
impl DebuggerCommand for NextCommand {
    fn handle(
        &self,
        context: &mut Context,
        _matches: &ArgMatches,
    ) -> Result<(bool, Option<String>), String> {
        // We execute directly instead of setting `Context::break_requested` so as not to interfere
        // with anything else that might be requesting a break, or in case debugger sessions become
        // stateful.
        context.execute_single_step();
        Ok((false, None))
    }
    fn extend(&self, command: Command) -> Command {
        next::Args::augment_subcommands(command)
    }
}

struct BreakpointCommand;
/// Adds a new debugger breakpoint at t
impl DebuggerCommand for BreakpointCommand {
    fn handle(
        &self,
        context: &mut Context,
        matches: &ArgMatches,
    ) -> Result<(bool, Option<String>), String> {
        let args = breakpoint::Args::from_arg_matches(matches).unwrap();
        match run_ext_api::<breakpoint::Api>(context, &args) {
            Err(IxaError::IxaError(e)) => Ok((false, Some(format!("error: {e}")))),
            Ok(return_value) => {
                match return_value {
                    breakpoint::Retval::List(bp_list) => {
                        let mut msg = format!("Scheduled breakpoints: {}\n", bp_list.len());
                        for bp in bp_list {
                            _ = writeln!(&mut msg, "\t{bp}");
                        }
                        return Ok((false, Some(msg)));
                    }
                    breakpoint::Retval::Ok => { /* pass */ }
                }

                Ok((false, None))
            }
            _ => unimplemented!(),
        }
    }
    fn extend(&self, command: Command) -> Command {
        breakpoint::Args::augment_subcommands(command)
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

fn exit_debugger() -> ! {
    println!("Got Ctrl-D, Exiting...");
    std::process::exit(0);
}

/// Starts a debugging REPL session, interrupting the normal simulation event loop.
#[allow(clippy::missing_panics_doc)]
pub fn enter_debugger(context: &mut Context) {
    let current_time = context.get_current_time();
    context.cancel_debugger_request();

    // We temporarily swap out the debugger so we can have simultaneous mutable access to
    // it and to `context`. We swap it back in at the end of the function.
    let mut debugger = context.get_data_mut(DebuggerPlugin).take().unwrap();

    println!("Debugging simulation at t={current_time}");
    loop {
        let line = match debugger.rl.readline(&format!("t={current_time:.4} $ ")) {
            Ok(line) => line,
            Err(
                rustyline::error::ReadlineError::WindowResized
                | rustyline::error::ReadlineError::Interrupted,
            ) => continue,
            Err(rustyline::error::ReadlineError::Eof) => exit_debugger(),
            Err(err) => panic!("Read error: {err}"),
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
                if let Some(message) = message {
                    println!("{message}");
                }
                if quit {
                    break;
                }
            }
            Err(err) => {
                eprintln!("{err}");
            }
        }
    }

    // Restore the debugger
    let saved_debugger = context.get_data_mut(DebuggerPlugin);
    *saved_debugger = Some(debugger);
}

#[cfg(test)]
mod tests {
    use super::{enter_debugger, DebuggerPlugin};
    use crate::{define_global_property, define_person_property, ContextGlobalPropertiesExt};
    use crate::{Context, ContextPeopleExt, ExecutionPhase};
    use assert_approx_eq::assert_approx_eq;

    fn process_line(line: &str, context: &mut Context) -> (bool, Option<String>) {
        // Temporarily take the data container out of context so that
        // we can operate on context.
        let data_container = context.get_data_mut(DebuggerPlugin);
        let debugger = data_container.take().unwrap();

        let res = debugger.process_command(line, context).unwrap();
        let data_container = context.get_data_mut(DebuggerPlugin);
        *data_container = Some(debugger);
        res
    }

    define_global_property!(FooGlobal, String);
    define_global_property!(BarGlobal, u32);
    define_person_property!(Age, u8);
    define_person_property!(Smile, u32);

    #[test]
    fn test_cli_debugger_breakpoint_set() {
        let context = &mut Context::new();
        let (quits, _output) = process_line("breakpoint set 4.0\n", context);

        assert!(!quits, "should not exit");

        let list = context.list_breakpoints(0);
        assert_eq!(list.len(), 1);
        if let Some(schedule) = list.first() {
            assert_eq!(schedule.priority, ExecutionPhase::First);
            assert_eq!(schedule.plan_id, 0u64);
            assert_approx_eq!(schedule.time, 4.0f64);
        }
    }

    #[test]
    fn test_cli_debugger_breakpoint_list() {
        let context = &mut Context::new();

        context.schedule_debugger(1.0, None, Box::new(enter_debugger));
        context.schedule_debugger(2.0, Some(ExecutionPhase::First), Box::new(enter_debugger));
        context.schedule_debugger(3.0, Some(ExecutionPhase::Normal), Box::new(enter_debugger));
        context.schedule_debugger(4.0, Some(ExecutionPhase::Last), Box::new(enter_debugger));

        let expected = r"Scheduled breakpoints: 4
	0: t=1 (First)
	1: t=2 (First)
	2: t=3 (Normal)
	3: t=4 (Last)
";

        let (quits, output) = process_line("breakpoint list\n", context);

        assert!(!quits, "should not exit");
        assert!(output.is_some());
        assert_eq!(output.unwrap(), expected);
    }

    #[test]
    fn test_cli_debugger_breakpoint_delete_id() {
        let context = &mut Context::new();

        context.schedule_debugger(1.0, None, Box::new(enter_debugger));
        context.schedule_debugger(2.0, None, Box::new(enter_debugger));

        let (quits, _output) = process_line("breakpoint delete 0\n", context);
        assert!(!quits, "should not exit");
        let list = context.list_breakpoints(0);

        assert_eq!(list.len(), 1);
        if let Some(schedule) = list.first() {
            assert_eq!(schedule.priority, ExecutionPhase::First);
            assert_eq!(schedule.plan_id, 1u64);
            assert_approx_eq!(schedule.time, 2.0f64);
        }
    }

    #[test]
    fn test_cli_debugger_breakpoint_delete_all() {
        let context = &mut Context::new();

        context.schedule_debugger(1.0, None, Box::new(enter_debugger));
        context.schedule_debugger(2.0, None, Box::new(enter_debugger));

        let (quits, _output) = process_line("breakpoint delete --all\n", context);
        assert!(!quits, "should not exit");
        let list = context.list_breakpoints(0);
        assert_eq!(list.len(), 0);
    }

    #[test]
    fn test_cli_debugger_breakpoint_disable_enable() {
        let context = &mut Context::new();

        let (quits, _output) = process_line("breakpoint disable\n", context);
        assert!(!quits, "should not exit");
        assert!(!context.breakpoints_are_enabled());

        let (quits, _output) = process_line("breakpoint enable\n", context);
        assert!(!quits, "should not exit");
        assert!(context.breakpoints_are_enabled());
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

        // We can't use process_line here because we expect an error to be
        // returned rather than string output
        let debugger = context.get_data_mut(DebuggerPlugin).take().unwrap();
        let result = debugger.process_command(input, context);
        let data_container = context.get_data_mut(DebuggerPlugin);
        *data_container = Some(debugger);

        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .contains("required arguments were not provided"));
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
        let (quits, _) = process_line("next\n", context);
        assert!(!quits, "should not exit");
    }
}
