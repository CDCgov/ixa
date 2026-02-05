use super::utils::*;

pub fn dev() -> Result<()> {
    log::info("Running model in dev mode...")?;
    run_command_or_exit("cargo", &["run"], None);
    Ok(())
}
