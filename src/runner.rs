use std::path::{Path, PathBuf};
use std::str::FromStr;

use crate::context::Context;
#[cfg(feature = "debugger")]
use crate::debugger::enter_debugger;
use crate::error::IxaError;
use crate::global_properties::ContextGlobalPropertiesExt;
use crate::log::level_to_string_list;
#[cfg(feature = "progress_bar")]
use crate::progress::init_timeline_progress_bar;
use crate::random::ContextRandomExt;
use crate::report::ContextReportExt;
#[cfg(feature = "web_api")]
use crate::web_api::ContextWebApiExt;
use crate::{info, set_log_level, set_module_filters, warn, LevelFilter};
use clap::{ArgAction, Args, Command, FromArgMatches as _};
#[cfg(debug_assertions)]
use clap_markdown::{help_markdown_command_custom, MarkdownOptions};

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
    #[cfg(debug_assertions)]
    /// Print help in Markdown format. This is enabled only for debug builds. Run an example with
    /// `--markdown-help`, and the file `docs/cli-usage.md` will be written. This file is then
    /// included in the crate-level docs. See `src/lib.rs`.
    #[arg(long, hide = true)]
    markdown_help: bool,

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

    #[arg(
        short,
        long,
        action = ArgAction::Count,
        long_help = r#"Increase logging verbosity (-v, -vv, -vvv, etc.)

  ┌─────────┬───────┬──────┬───────┬───────┬───────┐
  │ Level   │ ERROR │ WARN │ INFO  │ DEBUG │ TRACE │
  ├─────────┼───────┼──────┼───────┼───────┼───────┤
  │ Default │   ✓   │      │       │       │       │
  │ -v      │   ✓   │  ✓   │   ✓   │       │       │
  │ -vv     │   ✓   │  ✓   │   ✓   │   ✓   │       │
  │ -vvv    │   ✓   │  ✓   │   ✓   │   ✓   │   ✓   │
  └─────────┴───────┴──────┴───────┴───────┴───────┘
"#)]
    pub verbose: u8,

    /// Set logging to WARN level. Shortcut for `--log-level warn`.
    #[arg(long)]
    pub warn: bool,

    /// Set logging to DEBUG level. Shortcut for `--log-level DEBUG`.
    #[arg(long)]
    pub debug: bool,

    /// Set logging to TRACE level. Shortcut for `--log-level TRACE`.
    #[arg(long)]
    pub trace: bool,

    /// Set a breakpoint at a given time and start the debugger. Defaults to t=0.0
    #[arg(short, long)]
    pub debugger: Option<Option<f64>>,

    /// Enable the Web API at a given time. Defaults to t=0.0
    #[arg(short, long)]
    pub web: Option<Option<u16>>,

    /// Enable the timeline progress bar with a maximum time.
    #[arg(short, long)]
    pub timeline_progress_max: Option<f64>,

    /// Suppresses the printout of summary statistics at the end of the simulation.
    #[arg(long)]
    pub no_stats: bool,
}

impl BaseArgs {
    fn new() -> Self {
        BaseArgs {
            #[cfg(debug_assertions)]
            markdown_help: false,
            random_seed: 0,
            config: None,
            output_dir: None,
            file_prefix: None,
            force_overwrite: false,
            log_level: None,
            verbose: 0,
            warn: false,
            debug: false,
            trace: false,
            debugger: None,
            web: None,
            timeline_progress_max: None,
            no_stats: false,
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
///   a `Option<A>` where `A` is the custom cli arguments struct
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
    #[cfg(debug_assertions)]
    // Print help in markdown format
    if args.markdown_help {
        let cli = create_ixa_cli();
        let md_options = MarkdownOptions::new()
            .show_footer(false)
            .show_aliases(true)
            .show_table_of_contents(false)
            .title("Command Line Usage".to_string());
        let markdown = help_markdown_command_custom(&cli, &md_options);
        let path =
            PathBuf::from(option_env!("CARGO_WORKSPACE_DIR").unwrap_or(env!("CARGO_MANIFEST_DIR")))
                .join("docs")
                .join("cli-usage.md");
        std::fs::write(&path, markdown).unwrap_or_else(|e| {
            panic!(
                "Failed to write CLI help Markdown to file {}: {}",
                path.display(),
                e
            );
        });
    }

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

    // The default log level. We process the arguments first and then set the log level once.
    // We use the _maximum_ log level set by the user arguments if multiple log level flags
    // are provided.
    let mut current_log_level = crate::log::DEFAULT_LOG_LEVEL;

    // Explicitly setting the log level takes precedence over `-v`-style verbosity.
    if let Some(log_level) = args.log_level.as_ref() {
        if let Ok(level) = LevelFilter::from_str(log_level) {
            current_log_level = level;
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
    }

    // Process `-v`-style verbosity arguments.
    if args.verbose > 0 {
        let new_level = match args.verbose {
            1 => LevelFilter::Info,
            2 => LevelFilter::Debug,
            _ => LevelFilter::Trace,
        };
        current_log_level = current_log_level.max(new_level);
    }

    // Process "shortcut" log level arguments `--warn`, `--debug`, `--trace`.
    if args.warn {
        current_log_level = current_log_level.max(LevelFilter::Warn);
    }
    if args.debug {
        current_log_level = current_log_level.max(LevelFilter::Debug);
    }
    if args.trace {
        current_log_level = LevelFilter::Trace;
    }

    // Tell the user what log level they have enabled.
    let binary_name = std::env::args().next();
    let binary_name = binary_name
        .as_deref()
        .map(Path::new)
        .and_then(Path::file_name)
        .and_then(|s| s.to_str())
        .unwrap_or("[model]");
    println!(
        "Current log levels enabled: {}",
        level_to_string_list(current_log_level)
    );
    println!("Run {binary_name} --help -v to see more options");

    // Finally, set the log level to the computed max.
    if current_log_level != crate::log::DEFAULT_LOG_LEVEL {
        set_log_level(current_log_level);
    }

    context.init_random(args.random_seed);

    // If a breakpoint is provided, stop at that time
    #[cfg(feature = "debugger")]
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
    #[cfg(not(feature = "debugger"))]
    if args.debugger.is_some() {
        warn!("Ixa was not compiled with the debugger feature, but a debugger option was provided");
    }

    // If the Web API is provided, stop there.
    #[cfg(feature = "web_api")]
    if let Some(t) = args.web {
        let port = t.unwrap_or(33334);
        let url = context.setup_web_api(port).unwrap();
        println!("Web API active on {url}");
        context.schedule_web_api(0.0);
    }
    #[cfg(not(feature = "web_api"))]
    if args.web.is_some() {
        warn!("Ixa was not compiled with the web_api feature, but a web_api option was provided");
    }

    if let Some(max_time) = args.timeline_progress_max {
        // We allow a `max_time` of `0.0` to mean "disable timeline progress bar".
        if cfg!(not(feature = "progress_bar")) && max_time > 0.0 {
            warn!("Ixa was not compiled with the progress_bar feature, but a progress_bar option was provided");
        } else if max_time < 0.0 {
            warn!("timeline progress maximum must be nonnegative");
        }
        #[cfg(feature = "progress_bar")]
        if max_time > 0.0 {
            println!("ProgressBar max set to {}", max_time);
            init_timeline_progress_bar(max_time);
        }
    }

    if args.no_stats {
        context.print_execution_statistics = false;
    } else {
        if cfg!(target_family = "wasm") {
            warn!("the print-stats option is enabled; some statistics are not supported for the wasm target family");
        }
        context.print_execution_statistics = true;
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
