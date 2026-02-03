use std::fs;
use std::path::Path;

use super::utils::*;

const PROFILING_PROFILE: &str = r#"
[profile.profiling]
inherits = "release"
debug = true
"#;

pub fn profile() -> Result<()> {
    let cargo_toml_path = Path::new("Cargo.toml");

    // Check if Cargo.toml exists
    if !cargo_toml_path.exists() {
        anyhow::bail!("Cargo.toml not found. Are you in a Rust project directory?");
    }

    // Read Cargo.toml and check if profiling profile exists
    let cargo_toml = fs::read_to_string(cargo_toml_path)?;

    if !cargo_toml.contains("[profile.profiling]") {
        log::warning("No [profile.profiling] section found in Cargo.toml")?;

        let add_profile = confirm("Would you like to add a profiling profile?")
            .initial_value(true)
            .interact()?;

        if add_profile {
            // Append the profiling profile to Cargo.toml
            let updated_cargo_toml = format!("{}{}", cargo_toml, PROFILING_PROFILE);
            fs::write(cargo_toml_path, updated_cargo_toml)?;
            log::success("Added [profile.profiling] to Cargo.toml")?;
        } else {
            anyhow::bail!("Profiling profile is required to run this command");
        }
    }

    log::info("Running model with profiling enabled...")?;
    run_command_or_exit("cargo", &["run", "--profile", "profiling"], None);

    Ok(())
}
