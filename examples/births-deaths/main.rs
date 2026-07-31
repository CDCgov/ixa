use std::path::PathBuf;

use clap::Args;
use ixa::runner::run_with_custom_args;
use ixa_example_births_deaths::{try_initialize, try_initialize_with_parameters};

#[derive(Args, Debug)]
struct BirthsDeathsArgs {
    /// Path to a JSON parameters file; defaults to the model's bundled parameters.
    #[arg(long, value_name = "PATH")]
    parameters: Option<PathBuf>,
}

fn default_output_path() -> PathBuf {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    if manifest_dir.ends_with("births-deaths") {
        manifest_dir.join("output")
    } else {
        manifest_dir
            .join("examples")
            .join("births-deaths")
            .join("output")
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    run_with_custom_args::<BirthsDeathsArgs, _>(|context, base_args, custom_args| {
        let output_path = base_args.output_dir.unwrap_or_else(default_output_path);
        let parameters_path = custom_args.and_then(|args| args.parameters);
        if let Some(parameters_path) = parameters_path.as_deref() {
            try_initialize_with_parameters(context, &output_path, parameters_path).map_err(
                |error| {
                    eprintln!(
                        "failed to initialize births-deaths with parameters from {}: {error}",
                        parameters_path.display()
                    );
                    error
                },
            )
        } else {
            try_initialize(context, &output_path)
        }
    })?;
    Ok(())
}
