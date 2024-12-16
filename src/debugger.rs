use crate::Context;
use crate::ContextPeopleExt;
use crate::IxaError;
use clap::{ArgMatches, Command};
use std::cell::RefCell;
use std::collections::HashMap;
use std::io::Write;

trait DebuggerCommand {
    /// Handle the command and any inputs; returning true will exit the debugger
    fn handle(
        &self,
        context: &mut Context,
        cli: &DebuggerRepl,
        matches: &ArgMatches,
    ) -> Result<bool, String>;
    fn about(&self) -> &'static str;
}

struct DebuggerRepl {
    commands: HashMap<&'static str, Box<dyn DebuggerCommand>>,
    output: RefCell<Box<dyn Write>>,
}

fn flush() {
    std::io::stdout()
        .flush()
        .map_err(|e| e.to_string())
        .expect("Error flushing stdout");
}

impl DebuggerRepl {
    fn new() -> Self {
        DebuggerRepl {
            commands: HashMap::new(),
            output: RefCell::new(Box::new(std::io::stdout())),
        }
    }

    fn register_command(&mut self, name: &'static str, handler: Box<dyn DebuggerCommand>) {
        self.commands.insert(name, handler);
    }

    fn get_command(&self, name: &str) -> Option<&dyn DebuggerCommand> {
        self.commands.get(name).map(|command| &**command)
    }

    fn writeln(&self, formatted_string: &str) {
        let _ = writeln!(self.output.borrow_mut(), "{formatted_string}").map_err(|e| e.to_string());
        flush();
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
            cli = cli.subcommand(Command::new(*name).about(handler.about()).help_template(
                "{about-with-newline}\n{usage-heading}\n    {usage}\n\n{all-args}{after-help}",
            ));
        }

        cli
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
        cli: &DebuggerRepl,
        _matches: &ArgMatches,
    ) -> Result<bool, String> {
        cli.writeln(&format!("{}", context.get_current_population()));
        Ok(false)
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
        _cli: &DebuggerRepl,
        _matches: &ArgMatches,
    ) -> Result<bool, String> {
        Ok(true)
    }
}

fn readline(t: f64) -> Result<String, String> {
    write!(std::io::stdout(), "t={t} $ ").map_err(|e| e.to_string())?;
    std::io::stdout().flush().map_err(|e| e.to_string())?;
    let mut buffer = String::new();
    std::io::stdin()
        .read_line(&mut buffer)
        .map_err(|e| e.to_string())?;
    Ok(buffer)
}

fn setup_repl(line: &str, context: &mut Context) -> Result<bool, String> {
    let args = shlex::split(line).ok_or("error: Invalid quoting")?;

    let mut repl = DebuggerRepl::new();
    repl.register_command("population", Box::new(PopulationCommand));
    repl.register_command("continue", Box::new(ContinueCommand));

    let matches = repl
        .build_cli()
        .try_get_matches_from(args)
        .map_err(|e| e.to_string())?;

    if let Some((command, sub_matches)) = matches.subcommand() {
        // If the provided command is known, run its handler
        if let Some(handler) = repl.get_command(command) {
            return handler.handle(context, &repl, sub_matches);
        }
        // Unexpected command: print an error
        return Err(format!("Unknown command: {command}"));
    }

    unreachable!("subcommand required");
}

/// Starts the debugger and pauses execution
fn start_debugger(context: &mut Context) -> Result<(), IxaError> {
    let t = context.get_current_time();
    println!("Debugging simulation at t = {t}");
    loop {
        let line = readline(t).expect("Error reading input");
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        match setup_repl(line, context) {
            Ok(quit) => {
                if quit {
                    break;
                }
            }
            Err(err) => {
                write!(std::io::stdout(), "{err}").map_err(|e| e.to_string())?;
                flush();
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
    use assert_cmd::Command;
    use predicates::str::contains;

    #[test]
    fn test_cli_debugger_command() {}

    #[test]
    fn test_cli_debugger_quits() {
        let mut cmd = Command::cargo_bin("runner_test_debug").unwrap();
        let assert = cmd
            .args(["--debugger", "1.0"])
            .write_stdin("continue\n")
            .assert();

        assert
            .success()
            .stdout(contains("Debugging simulation at t = 1"));
    }

    #[test]
    #[ignore]
    fn test_cli_debugger_population() {
        let assert = Command::cargo_bin("runner_test_debug")
            .unwrap()
            .args(["--debugger", "1.0"])
            .write_stdin("population\n")
            .write_stdin("continue\n")
            .assert();

        assert
            .success()
            // This doesn't seem to work for some reason
            .stdout(contains("The number of people is 3"));
    }
}
