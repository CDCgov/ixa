// Re-export dependencies
#![allow(unused_imports)]
use std::path::Path;
use std::process::{Command, ExitStatus, Stdio};

pub use anyhow::Result;
pub use cliclack::{
    confirm, input, log, multi_progress, multiselect, outro, progress_bar, select, spinner,
};

/// Run a subprocess command, inheriting stdout/stderr for real-time output.
/// Returns the exit status of the command.
pub fn run_command(
    program: &str,
    args: &[&str],
    working_dir: Option<&Path>,
) -> std::io::Result<ExitStatus> {
    let mut cmd = Command::new(program);
    cmd.args(args)
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit());

    if let Some(dir) = working_dir {
        cmd.current_dir(dir);
    }

    cmd.status()
}

/// Run a subprocess command and exit the process if it fails.
pub fn run_command_or_exit(program: &str, args: &[&str], working_dir: Option<&Path>) {
    match run_command(program, args, working_dir) {
        Ok(status) => {
            if !status.success() {
                std::process::exit(status.code().unwrap_or(1));
            }
        }
        Err(e) => {
            eprintln!("Failed to execute {}: {}", program, e);
            std::process::exit(1);
        }
    }
}
