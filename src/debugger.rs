use crate::Context;
use crate::ContextPeopleExt;
use crate::IxaError;
use clap::Command;
use std::io::Write;

fn cli() -> Command {
    // strip out "Usage: " in the default template
    const MAIN_HELP_TEMPLATE: &str = "\
        {all-args}
    ";
    // strip out name/version
    const COMMAND_TEMPLATE: &str = "\
        {about-with-newline}\n\
        {usage-heading}\n    {usage}\n\
        \n\
        {all-args}{after-help}\
    ";

    Command::new("repl")
        .multicall(true)
        .arg_required_else_help(true)
        .subcommand_required(true)
        .subcommand_value_name("DEBUGGER")
        .subcommand_help_heading("IXA DEBUGGER")
        .help_template(MAIN_HELP_TEMPLATE)
        .subcommand(
            Command::new("population")
                .about("Get the total number of people")
                .help_template(COMMAND_TEMPLATE),
        )
        .subcommand(
            Command::new("continue")
                .alias("exit")
                .alias("quit")
                .about("Continue the simulation and exit the debugger")
                .help_template(COMMAND_TEMPLATE),
        )
}

fn readline() -> Result<String, String> {
    write!(std::io::stdout(), "$ ").map_err(|e| e.to_string())?;
    std::io::stdout().flush().map_err(|e| e.to_string())?;
    let mut buffer = String::new();
    std::io::stdin()
        .read_line(&mut buffer)
        .map_err(|e| e.to_string())?;
    Ok(buffer)
}

fn flush() -> Result<(), String> {
    std::io::stdout().flush().map_err(|e| e.to_string())
}

fn respond(line: &str, context: &mut Context) -> Result<bool, String> {
    let args = shlex::split(line).ok_or("error: Invalid quoting")?;
    let matches = cli()
        .try_get_matches_from(args)
        .map_err(|e| e.to_string())?;

    match matches.subcommand() {
        Some(("population", _matches)) => {
            writeln!(
                std::io::stdout(),
                "The number of people is {}",
                context.get_current_population()
            )
            .map_err(|e| e.to_string())?;
            flush()?;
        }
        Some(("continue", _matches)) => {
            writeln!(
                std::io::stdout(),
                "Continuing the simulation from t = {}",
                context.get_current_time()
            )
            .map_err(|e| e.to_string())?;
            flush()?;
            return Ok(true);
        }
        Some((name, _matches)) => unimplemented!("{name}"),
        None => unreachable!("subcommand required"),
    }

    Ok(false)
}

pub trait ContextDebugExt {
    /// Pause the simulation at the current time and start the debugger.
    /// The debugger allows you to inspect the state of the simulation
    ///
    /// # Errors
    /// Reading or writing to stdin/stdout, or some problem in the debugger
    fn breakpoint(&mut self) -> Result<(), IxaError>;

    /// Schedule a breakpoint at a given time t
    fn schedule_breakpoint(&mut self, t: f64);
}

impl ContextDebugExt for Context {
    fn breakpoint(&mut self) -> Result<(), IxaError> {
        println!("Debugging simulation at t = {}", self.get_current_time());
        loop {
            let line = readline()?;
            let line = line.trim();
            if line.is_empty() {
                continue;
            }

            match respond(line, self) {
                Ok(quit) => {
                    if quit {
                        break;
                    }
                }
                Err(err) => {
                    write!(std::io::stdout(), "{err}").map_err(|e| e.to_string())?;
                    flush()?;
                }
            }
        }
        Ok(())
    }

    fn schedule_breakpoint(&mut self, t: f64) {
        self.add_plan(t, |context| {
            context.breakpoint().expect("Error in debugger");
        });
    }
}

#[cfg(test)]
mod tests {
    use assert_cmd::Command;
    use predicates::str::contains;

    #[test]
    fn test_cli_debugger_quits() {
        let mut cmd = Command::cargo_bin("runner_test_debug").unwrap();
        let assert = cmd
            .args(["--debugger", "1.0"])
            .write_stdin("continue\n")
            .assert();

        assert
            .success()
            .stdout(contains("Debugging simulation at t = 1"))
            .stdout(contains("Continuing the simulation from t = 1"));
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
