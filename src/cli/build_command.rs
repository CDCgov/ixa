use super::utils::*;

pub fn build() -> Result<()> {
    log::info("Building model in release mode...")?;
    run_command_or_exit("cargo", &["build", "--release"], None);
    log::success("Build complete!")?;
    Ok(())
}
