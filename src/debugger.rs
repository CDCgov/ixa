use crate::Context;
use crate::ContextPeopleExt;
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
            Command::new("step")
                .about("Advance the simulation by 1.0 and break")
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
        Some(("step", _matches)) => {
            let next_t = context.get_current_time() + 1.0;
            context.schedule_breakpoint(next_t);
            flush()?;
            return Ok(true);
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
    fn breakpoint(&mut self) -> Result<(), String>;

    /// Schedule a breakpoint at a given time t
    fn schedule_breakpoint(&mut self, t: f64);
}

impl ContextDebugExt for Context {
    fn breakpoint(&mut self) -> Result<(), String> {
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
