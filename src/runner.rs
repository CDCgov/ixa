use std::path::{Path, PathBuf};

use crate::context::Context;
use crate::error::IxaError;
use crate::global_properties::ContextGlobalPropertiesExt;
use crate::random::ContextRandomExt;
use crate::report::ContextReportExt;
use clap::{Args, Command, FromArgMatches as _};

#[derive(Args, Debug)]
pub struct BaseArgs {
    /// Random seed
    #[arg(short, long, default_value = "0")]
    pub seed: u64,

    /// Optional path for a global properties config file
    #[arg(short, long, default_value = "")]
    pub config: String,

    /// Optional path for report output
    #[arg(short, long, default_value = "")]
    pub output_dir: String,
}

#[derive(Args)]
pub struct PlaceholderCustom {}

#[allow(clippy::missing_errors_doc)]
pub fn run_with_custom_args<A, F>(load: F) -> Result<(), Box<dyn std::error::Error>>
where
    A: Args,
    F: Fn(&mut Context, BaseArgs, Option<A>) -> Result<(), IxaError>,
{
    let cli = Command::new("Ixa");
    let cli = BaseArgs::augment_args(cli);
    let cli = A::augment_args(cli);
    let matches = cli.get_matches();

    let base_args_matches = BaseArgs::from_arg_matches(&matches)?;
    let custom_matches = A::from_arg_matches(&matches)?;

    run_with_args_internal(base_args_matches, Some(custom_matches), load)
}

#[allow(clippy::missing_errors_doc)]
pub fn run_with_args<F>(load: F) -> Result<(), Box<dyn std::error::Error>>
where
    F: Fn(&mut Context, BaseArgs, Option<PlaceholderCustom>) -> Result<(), IxaError>,
{
    let cli = Command::new("Ixa");
    let cli = BaseArgs::augment_args(cli);
    let matches = cli.get_matches();

    let base_args_matches = BaseArgs::from_arg_matches(&matches)?;

    run_with_args_internal(base_args_matches, None, load)
}

fn setup_context(args: &BaseArgs) -> Result<Context, IxaError> {
    // Instantiate a context
    let mut context = Context::new();

    // Optionally set global properties from a file
    if !args.config.is_empty() {
        println!("Loading global properties from: {}", args.config);
        let config_path = Path::new(&args.config);
        context.load_global_properties(config_path)?;
    }

    // Optionally set output dir for reports
    if !args.output_dir.is_empty() {
        let output_dir = PathBuf::from(&args.output_dir);
        let report_config = context.report_options();
        report_config.directory(output_dir);
    }

    context.init_random(args.seed);
    Ok(context)
}

fn run_with_args_internal<A, F>(
    args: BaseArgs,
    custom_args: Option<A>,
    load: F,
) -> Result<(), Box<dyn std::error::Error>>
where
    A: Args,
    F: Fn(&mut Context, BaseArgs, Option<A>) -> Result<(), IxaError>,
{
    // Create a context
    let mut context = setup_context(&args)?;

    // Run the provided Fn
    load(&mut context, args, custom_args)?;

    // Execute the context
    context.execute();
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::define_global_property;
    use serde::Deserialize;

    #[derive(Args, Debug)]
    struct CustomArgs {
        #[arg(short, long, default_value = "0")]
        field: u32,
    }

    #[test]
    fn test_run_with_custom_args() {
        let result = run_with_custom_args(|_, _, _: Option<CustomArgs>| Ok(()));
        assert!(result.is_ok());
    }

    #[test]
    fn test_run_with_args() {
        let result = run_with_args(|_, _, _| Ok(()));
        assert!(result.is_ok());
    }

    #[test]
    fn test_run_with_random_seed() {
        let test_args = BaseArgs {
            seed: 42,
            config: String::new(),
            output_dir: String::new(),
        };
        let result = run_with_args_internal(test_args, None, |ctx, _, _: Option<CustomArgs>| {
            assert_eq!(ctx.get_base_seed(), 42);
            Ok(())
        });
        assert!(result.is_ok());
    }

    #[derive(Deserialize)]
    pub struct RunnerPropertyType {
        field_int: u32,
    }
    define_global_property!(RunnerProperty, RunnerPropertyType);

    #[test]
    fn test_run_with_config_path() {
        let test_args = BaseArgs {
            seed: 42,
            config: "tests/data/global_properties_runner.json".to_string(),
            output_dir: String::new(),
        };
        let result = run_with_args_internal(test_args, None, |ctx, _, _: Option<CustomArgs>| {
            let p3 = ctx.get_global_property_value(RunnerProperty).unwrap();
            assert_eq!(p3.field_int, 0);
            Ok(())
        });
        assert!(result.is_ok());
    }

    #[test]
    fn test_run_with_output_dir() {
        let test_args = BaseArgs {
            seed: 42,
            config: String::new(),
            output_dir: "data".to_string(),
        };
        let result = run_with_args_internal(test_args, None, |ctx, _, _: Option<CustomArgs>| {
            let output_dir = &ctx.report_options().directory;
            assert_eq!(output_dir, &PathBuf::from("data"));
            Ok(())
        });
        assert!(result.is_ok());
    }
}
