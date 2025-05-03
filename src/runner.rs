use std::path::PathBuf;
use std::str::FromStr;

use crate::error::IxaError;
use crate::global_properties::ContextGlobalPropertiesExt;
use crate::random::ContextRandomExt;
use crate::report::ContextReportExt;
use crate::{context::Context, web_api::ContextWebApiExt};
use crate::{info, set_log_level, set_module_filters, LevelFilter};

use crate::debugger::enter_debugger;
use clap::{Args, Command, FromArgMatches as _};

/// Custom parser for log levels
fn parse_log_levels(s: &str) -> Result<Vec<(String, LevelFilter)>, String> {
    s.split(',')
        .map(|pair| {
            let mut iter = pair.split('=');
            let key = iter
                .next()
                .ok_or_else(|| format!("Invalid key in pair: {pair}"))?;
            let value = iter
                .next()
                .ok_or_else(|| format!("Invalid value in pair: {pair}"))?;
            let level =
                LevelFilter::from_str(value).map_err(|_| format!("Invalid log level: {value}"))?;
            Ok((key.to_string(), level))
        })
        .collect()
}

/// Default cli arguments for ixa runner
#[derive(Args, Debug)]
pub struct BaseArgs {
    /// Random seed
    #[arg(short, long, default_value = "0")]
    pub random_seed: u64,

    /// Optional path for a global properties config file
    #[arg(short, long)]
    pub config: Option<PathBuf>,

    /// Optional path for report output
    #[arg(short, long = "output")]
    pub output_dir: Option<PathBuf>,

    /// Optional prefix for report files
    #[arg(long = "prefix")]
    pub file_prefix: Option<String>,

    /// Overwrite existing report files?
    #[arg(short, long)]
    pub force_overwrite: bool,

    /// Enable logging
    #[arg(short, long)]
    pub log_level: Option<String>,

    /// Set a breakpoint at a given time and start the debugger. Defaults to t=0.0
    #[arg(short, long)]
    pub debugger: Option<Option<f64>>,

    /// Enable the Web API at a given time. Defaults to t=0.0
    #[arg(short, long)]
    pub web: Option<Option<u16>>,
}

impl BaseArgs {
    fn new() -> Self {
        BaseArgs {
            random_seed: 0,
            config: None,
            output_dir: None,
            file_prefix: None,
            force_overwrite: false,
            log_level: None,
            debugger: None,
            web: None,
        }
    }
}

impl Default for BaseArgs {
    fn default() -> Self {
        BaseArgs::new()
    }
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
///   a Option<A> where A is the custom cli arguments struct
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
/// - `setup_fn`: A function that takes a mutable reference to a `Context` and `BaseArgs` struct
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
    if args.config.is_some() {
        let config_path = args.config.clone().unwrap();
        println!("Loading global properties from: {config_path:?}");
        context.load_global_properties(&config_path)?;
    }

    // Configure report options
    let report_config = context.report_options();
    if args.output_dir.is_some() {
        report_config.directory(args.output_dir.clone().unwrap());
    }
    if args.file_prefix.is_some() {
        report_config.file_prefix(args.file_prefix.clone().unwrap());
    }
    if args.force_overwrite {
        report_config.overwrite(true);
    }
    if let Some(log_level) = args.log_level.as_ref() {
        if let Ok(level) = LevelFilter::from_str(log_level) {
            set_log_level(level);
            info!("Logging enabled at level {level}");
        } else if let Ok(log_levels) = parse_log_levels(log_level) {
            let log_levels_slice: Vec<(&String, LevelFilter)> =
                log_levels.iter().map(|(k, v)| (k, *v)).collect();
            set_module_filters(log_levels_slice.as_slice());
            for (key, value) in log_levels {
                println!("Logging enabled for {key} at level {value}");
                // Here you can set the log level for each key-value pair as needed
            }
        } else {
            return Err(format!("Invalid log level format: {log_level}").into());
        }
    } else {
        info!("Logging disabled.");
    }

    context.init_random(args.random_seed);

    // If a breakpoint is provided, stop at that time
    if let Some(t) = args.debugger {
        assert!(
            args.web.is_none(),
            "Cannot run with both the debugger and the Web API"
        );
        match t {
            None => {
                context.request_debugger();
            }
            Some(time) => {
                context.schedule_debugger(time, None, Box::new(enter_debugger));
            }
        }
    }

    // If the Web API is provided, stop there.
    if let Some(t) = args.web {
        let port = t.unwrap_or(33334);
        let url = context.setup_web_api(port).unwrap();
        println!("Web API active on {url}");
        context.schedule_web_api(0.0);
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
    use serde::{Deserialize, Serialize};

    #[derive(Args, Debug)]
    struct CustomArgs {
        #[arg(short, long, default_value = "0")]
        a: u32,
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
            random_seed: 42,
            ..Default::default()
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

    #[derive(Serialize, Deserialize)]
    pub struct RunnerPropertyType {
        field_int: u32,
    }
    define_global_property!(RunnerProperty, RunnerPropertyType);

    #[test]
    fn test_run_with_config_path() {
        let test_args = BaseArgs {
            config: Some(PathBuf::from("tests/data/global_properties_runner.json")),
            ..Default::default()
        };
        let result = run_with_args_internal(test_args, None, |ctx, _, _: Option<()>| {
            let p3 = ctx.get_global_property_value(RunnerProperty).unwrap();
            assert_eq!(p3.field_int, 0);
            Ok(())
        });
        assert!(result.is_ok());
    }

    #[test]
    fn test_run_with_report_options() {
        let test_args = BaseArgs {
            output_dir: Some(PathBuf::from("data")),
            file_prefix: Some("test".to_string()),
            force_overwrite: true,
            ..Default::default()
        };
        let result = run_with_args_internal(test_args, None, |ctx, _, _: Option<()>| {
            let opts = &ctx.report_options();
            assert_eq!(opts.output_dir, PathBuf::from("data"));
            assert_eq!(opts.file_prefix, "test".to_string());
            assert!(opts.overwrite);
            Ok(())
        });
        assert!(result.is_ok());
    }

    #[test]
    fn test_run_with_custom() {
        let test_args = BaseArgs::new();
        let custom = CustomArgs { a: 42 };
        let result = run_with_args_internal(test_args, Some(custom), |_, _, c| {
            assert_eq!(c.unwrap().a, 42);
            Ok(())
        });
        assert!(result.is_ok());
    }

    #[test]
    fn test_run_with_logging_enabled() {
        let mut test_args = BaseArgs::new();
        test_args.log_level = Some(LevelFilter::Info.to_string());
        let result = run_with_args_internal(test_args, None, |_, _, _: Option<()>| Ok(()));
        assert!(result.is_ok());
    }
}
