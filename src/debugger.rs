use crate::Context;
use crate::ContextPeopleExt;
use crate::IxaError;
use clap::value_parser;
use clap::{Arg, ArgMatches, Command};
use std::cell::RefCell;
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

struct DebuggerRepl {
    commands: HashMap<&'static str, Box<dyn DebuggerCommand>>,
    output: RefCell<Box<dyn Write>>,
}

impl DebuggerRepl {
    fn new(output: Box<dyn Write>) -> Self {
        DebuggerRepl {
            commands: HashMap::new(),
            output: RefCell::new(output),
        }
    }

    fn register_command(&mut self, name: &'static str, handler: Box<dyn DebuggerCommand>) {
        self.commands.insert(name, handler);
    }

    fn get_command(&self, name: &str) -> Option<&dyn DebuggerCommand> {
        self.commands.get(name).map(|command| &**command)
    }

    fn writeln(&self, formatted_string: &str) {
        let mut output = self.output.borrow_mut();
        writeln!(output, "{formatted_string}")
            .map_err(|e| e.to_string())
            .unwrap();
        output.flush().unwrap();
    }

    fn build_cli(&self) -> Command {
        let mut cli = Command::new("repl")
            .multicall(true)
            .arg_required_else_help(true)
            .subcommand_required(true)
            .subcommand_value_name("DEBUGGER")
            .subcommand_help_heading("IXA DEBUGGER")
            .help_template("{all-args}");

        for (name, handler) in &self.commands {
            let subcommand =
                handler.extend(Command::new(*name).about(handler.about()).help_template(
                    "{about-with-newline}\n{usage-heading}\n    {usage}\n\n{all-args}{after-help}",
                ));
            cli = cli.subcommand(subcommand);
        }

        cli
    }

    fn process_line(&self, l: &str, context: &mut Context) -> Result<bool, String> {
        let args = shlex::split(l).ok_or("Error splitting lines")?;
        let matches = self
            .build_cli()
            .try_get_matches_from(args)
            .map_err(|e| e.to_string())?;

        if let Some((command, sub_matches)) = matches.subcommand() {
            // If the provided command is known, run its handler
            if let Some(handler) = self.get_command(command) {
                let (quit, output) = handler.handle(context, sub_matches)?;
                if let Some(output) = output {
                    self.writeln(&output);
                }
                return Ok(quit);
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

// Assemble all the commands
fn build_repl<W: Write + 'static>(output: W) -> DebuggerRepl {
    let mut repl = DebuggerRepl::new(Box::new(output));

    repl.register_command("population", Box::new(PopulationCommand));
    repl.register_command("next", Box::new(NextCommand));
    repl.register_command("continue", Box::new(ContinueCommand));

    repl
}

// Helper function to read a line from stdin
fn readline(t: f64) -> Result<String, String> {
    write!(std::io::stdout(), "t={t} $ ").map_err(|e| e.to_string())?;
    std::io::stdout().flush().map_err(|e| e.to_string())?;
    let mut buffer = String::new();
    std::io::stdin()
        .read_line(&mut buffer)
        .map_err(|e| e.to_string())?;
    Ok(buffer)
}

/// Starts the debugger and pauses execution
fn start_debugger(context: &mut Context) -> Result<(), IxaError> {
    let t = context.get_current_time();
    let repl = build_repl(std::io::stdout());
    println!("Debugging simulation at t={t}");
    loop {
        let line = readline(t).expect("Error reading input");
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        match repl.process_line(line, context) {
            Ok(quit) => {
                if quit {
                    break;
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
        self.add_plan(t, |context| {
            start_debugger(context).expect("Error in debugger");
        });
    }
}

#[cfg(test)]
mod tests {
    use crate::{Context, ContextPeopleExt};
    use std::{cell::RefCell, io::Write, rc::Rc};

    use super::build_repl;

    #[derive(Clone)]
    struct StdoutMock {
        storage: Rc<RefCell<Vec<u8>>>,
    }

    impl StdoutMock {
        fn new() -> Self {
            StdoutMock {
                storage: Rc::new(RefCell::new(Vec::new())),
            }
        }
        fn into_inner(self) -> Vec<u8> {
            Rc::try_unwrap(self.storage)
                .expect("Multiple references to storage")
                .into_inner()
        }
        fn into_string(self) -> String {
            String::from_utf8(self.into_inner()).unwrap()
        }
    }
    impl Write for StdoutMock {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            self.storage.borrow_mut().write(buf)
        }
        fn flush(&mut self) -> std::io::Result<()> {
            self.storage.borrow_mut().flush()
        }
    }

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

        let output = StdoutMock::new();
        let repl = build_repl(output.clone());
        let quits = repl.process_line("population\n", context).unwrap();
        assert!(!quits, "should not exit");

        drop(repl);
        assert!(output.into_string().contains('2'));
    }

    #[test]
    fn test_cli_continue() {
        let context = &mut Context::new();
        let output = StdoutMock::new();
        let repl = build_repl(output.clone());
        let quits = repl.process_line("continue\n", context).unwrap();
        assert!(quits, "should exit");
    }

    #[test]
    fn test_cli_next() {
        let context = &mut Context::new();
        assert_eq!(context._remaining_plan_count(), 0);
        let output = StdoutMock::new();
        let repl = build_repl(output.clone());
        let quits = repl.process_line("next 2\n", context).unwrap();
        assert!(quits, "should exit");
        assert_eq!(
            context._remaining_plan_count(),
            1,
            "should schedule a plan for the debugger to pause"
        );
    }
}
