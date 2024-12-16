use std::path::{Path, PathBuf};

use crate::error::IxaError;
use crate::global_properties::ContextGlobalPropertiesExt;
use crate::random::ContextRandomExt;
use crate::report::ContextReportExt;
use crate::{context::Context, debugger::ContextDebugExt};
use clap::{Args, Command, FromArgMatches as _};

/// Default cli arguments for ixa runner
#[derive(Args, Debug)]
pub struct BaseArgs {
    /// Random seed
    #[arg(short, long, default_value = "0")]
    pub random_seed: u64,

    /// Optional path for a global properties config file
    #[arg(short, long, default_value = "")]
    pub config: String,

    /// Optional path for report output
    #[arg(short, long, default_value = "")]
    pub output_dir: String,

    /// Set a breakpoint at a given time and start the debugger. Defaults to t=0.0
    #[arg(short, long)]
    pub debugger: Option<Option<f64>>,
}

#[derive(Args)]
pub struct PlaceholderCustom {}

fn create_ixa_cli() -> Command {
    let cli = Command::new("ixa");
    BaseArgs::augment_args(cli)
}

/// Runs a simulation with custom cli arguments.
///
/// This function allows you to define custom arguments and a setup function
///
/// # Parameters
/// - `setup_fn`: A function that takes a mutable reference to a `Context`, a `BaseArgs` struct,
///    a Option<A> where A is the custom cli arguments struct
///
/// # Errors
/// Returns an error if argument parsing or the setup function fails
#[allow(clippy::missing_errors_doc)]
pub fn run_with_custom_args<A, F>(setup_fn: F) -> Result<Context, Box<dyn std::error::Error>>
where
    A: Args,
    F: Fn(&mut Context, BaseArgs, Option<A>) -> Result<(), IxaError>,
{
    let mut cli = create_ixa_cli();
    cli = A::augment_args(cli);
    let matches = cli.get_matches();

    let base_args_matches = BaseArgs::from_arg_matches(&matches)?;
    let custom_matches = A::from_arg_matches(&matches)?;
    run_with_args_internal(base_args_matches, Some(custom_matches), setup_fn)
}

/// Runs a simulation with default cli arguments
///
/// This function parses command line arguments allows you to define a setup function
///
/// # Parameters
/// - `setup_fn`: A function that takes a mutable reference to a `Context`and `BaseArgs` struct
///
/// # Errors
/// Returns an error if argument parsing or the setup function fails
#[allow(clippy::missing_errors_doc)]
pub fn run_with_args<F>(setup_fn: F) -> Result<Context, Box<dyn std::error::Error>>
where
    F: Fn(&mut Context, BaseArgs, Option<PlaceholderCustom>) -> Result<(), IxaError>,
{
    let cli = create_ixa_cli();
    let matches = cli.get_matches();

    let base_args_matches = BaseArgs::from_arg_matches(&matches)?;
    run_with_args_internal(base_args_matches, None, setup_fn)
}

fn run_with_args_internal<A, F>(
    args: BaseArgs,
    custom_args: Option<A>,
    setup_fn: F,
) -> Result<Context, Box<dyn std::error::Error>>
where
    F: Fn(&mut Context, BaseArgs, Option<A>) -> Result<(), IxaError>,
{
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

    context.init_random(args.random_seed);

    // If a breakpoint is provided, stop at that time
    if let Some(t) = args.debugger {
        context.schedule_debugger(t.unwrap_or(0.0));
    }

    // Run the provided Fn
    setup_fn(&mut context, args, custom_args)?;

    // Execute the context
    context.execute();
    Ok(context)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{define_global_property, define_rng};
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
    fn test_cli_invocation_with_custom_args() {
        // Note this target is defined in the bin section of Cargo.toml
        // and the entry point is in tests/bin/runner_test_custom_args
        assert_cmd::Command::cargo_bin("runner_test_custom_args")
            .unwrap()
            .args(["--field", "42"])
            .assert()
            .success()
            .stdout("42\n");
    }

    #[test]
    fn test_run_with_args() {
        let result = run_with_args(|_, _, _| Ok(()));
        assert!(result.is_ok());
    }

    #[test]
    fn test_run_with_random_seed() {
        let test_args = BaseArgs {
            random_seed: 42,
            config: String::new(),
            output_dir: String::new(),
            debugger: None,
        };

        // Use a comparison context to verify the random seed was set
        let mut compare_ctx = Context::new();
        compare_ctx.init_random(42);
        define_rng!(TestRng);
        let result = run_with_args_internal(test_args, None, |ctx, _, _: Option<()>| {
            assert_eq!(
                ctx.sample_range(TestRng, 0..100),
                compare_ctx.sample_range(TestRng, 0..100)
            );
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
            random_seed: 42,
            config: "tests/data/global_properties_runner.json".to_string(),
            output_dir: String::new(),
            debugger: None,
        };
        let result = run_with_args_internal(test_args, None, |ctx, _, _: Option<()>| {
            let p3 = ctx.get_global_property_value(RunnerProperty).unwrap();
            assert_eq!(p3.field_int, 0);
            Ok(())
        });
        assert!(result.is_ok());
    }

    #[test]
    fn test_run_with_output_dir() {
        let test_args = BaseArgs {
            random_seed: 42,
            config: String::new(),
            output_dir: "data".to_string(),
            debugger: None,
        };
        let result = run_with_args_internal(test_args, None, |ctx, _, _: Option<()>| {
            let output_dir = &ctx.report_options().directory;
            assert_eq!(output_dir, &PathBuf::from("data"));
            Ok(())
        });
        assert!(result.is_ok());
    }

    #[test]
    fn test_run_with_custom() {
        let test_args = BaseArgs {
            random_seed: 42,
            config: String::new(),
            output_dir: String::new(),
            debugger: None,
        };
        let custom = CustomArgs { field: 42 };
        let result = run_with_args_internal(test_args, Some(custom), |_, _, c| {
            assert_eq!(c.unwrap().field, 42);
            Ok(())
        });
        assert!(result.is_ok());
    }
}
