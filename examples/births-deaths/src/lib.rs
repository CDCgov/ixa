use std::path::{Path, PathBuf};

use ixa::prelude::*;

pub mod demographics_report;
pub mod incidence_report;
pub mod infection_manager;
pub mod parameters_loader;
pub mod population_manager;
pub mod transmission_manager;

use crate::parameters_loader::Parameters;

/// Initializes the births-deaths model using its bundled parameter file.
///
/// # Errors
///
/// Returns an [`IxaError`] if the parameters or reports cannot be initialized.
pub fn try_initialize(context: &mut Context, output_path: &Path) -> Result<(), IxaError> {
    let parameters_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("input.json");
    try_initialize_with_parameters(context, output_path, &parameters_path)
}

/// Initializes the births-deaths model using `parameters_path`.
///
/// # Errors
///
/// Returns an [`IxaError`] if the parameters or reports cannot be initialized.
pub fn try_initialize_with_parameters(
    context: &mut Context,
    output_path: &Path,
    parameters_path: &Path,
) -> Result<(), IxaError> {
    parameters_loader::init_parameters(context, parameters_path)?;

    let parameters = context
        .get_global_property_value(Parameters)
        .cloned()
        .ok_or_else(|| IxaError::PropertyNotSet {
            name: "Parameters".to_string(),
        })?;
    context.init_random(parameters.seed);

    demographics_report::init(context, output_path)?;
    incidence_report::init(context, output_path)?;

    population_manager::init(context);
    transmission_manager::init(context);
    infection_manager::init(context);

    // Cap the run without keeping an otherwise completed simulation alive.
    context.schedule_shutdown(parameters.max_time);
    Ok(())
}

/// Initializes the births-deaths model using its bundled parameter file.
///
/// # Panics
///
/// Panics if the parameters cannot be loaded or initialized. New callers that
/// can propagate initialization failures should use [`try_initialize`].
pub fn initialize(context: &mut Context, output_path: &Path) {
    if let Err(error) = try_initialize(context, output_path) {
        panic!("failed to initialize births-deaths model: {error}");
    }
}
