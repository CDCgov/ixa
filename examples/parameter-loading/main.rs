use std::path::PathBuf;

use ixa::prelude::*;
mod parameters_loader;
use crate::parameters_loader::{Parameters, ParametersExt};

fn main() -> Result<(), IxaError> {
    let mut context = Context::new();

    // Initialize parameters as global properties
    let file_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("examples")
        .join("parameter-loading")
        .join("input.json");
    context.init_parameters(&file_path)?;

    // Get a parameter
    let &Parameters { seed, .. } = context.get_parameters();
    context.init_random(seed);

    Ok(())
}
