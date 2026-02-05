use std::path::Path;

use super::utils::*;

pub fn new(path: &Path) -> Result<()> {
    let path_str = path.to_string_lossy();

    log::info(format!("Creating new 🦀 ixa project in {path_str}").to_string())?;

    // Check if the directory exists
    if path.exists() {
        // Ask user if they want to overwrite the directory
        let overwrite = confirm("This directory already exists. Do you want to delete it first?")
            .initial_value(false)
            .interact()?;
        if overwrite {
            // Remove the directory
            log::info("Removing directory...")?;
            std::fs::remove_dir_all(path)?;
        } else {
            return Err(anyhow::anyhow!("Directory already exists"));
        }
    }

    run_command_or_exit("cargo", &["new", &path_str], None);
    run_command_or_exit("cargo", &["add", "ixa"], Some(path));

    // TODO port creation script here

    log::success("Your new project was successfully created!")?;
    outro("To start developing your model, run:")?;
    print!("\ncd {path_str} && ixa dev\n\n");

    Ok(())
}
