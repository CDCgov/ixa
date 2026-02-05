use super::utils::*;

pub fn run() -> Result<()> {
    log::info("Running model in release mode...")?;
    run_command_or_exit("cargo", &["run", "--release"], None);
    Ok(())
}
