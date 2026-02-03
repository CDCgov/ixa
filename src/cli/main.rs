use std::path::PathBuf;

use anyhow::Result;
use clap::{Parser, Subcommand};

use super::commands;

#[derive(Parser)]
#[command(name = "ixa")]
#[command(about = "A CLI tool for building and running ixa agent-based models")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Create a new ixa project in the specified directory
    New {
        /// Directory to create the new project in
        path: PathBuf,
    },
    /// Build the model with cargo (release mode)
    Build,
    /// Run the model in release mode (from the target)
    Run,
    /// Run the model in dev mode (cargo run)
    Dev,
    /// Profile the model
    Profile,
}

pub fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::New { path } => commands::new(&path),
        Commands::Build => commands::build(),
        Commands::Run => commands::run(),
        Commands::Dev => commands::dev(),
        Commands::Profile => commands::profile(),
    }
}
