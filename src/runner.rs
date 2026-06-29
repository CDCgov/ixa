use std::path::{Path, PathBuf};
use std::str::FromStr;

use clap::error::ErrorKind as ClapErrorKind;
use clap::parser::ValueSource;
use clap::{ArgAction, ArgMatches, Args, Command, FromArgMatches as _};
#[cfg(feature = "write_cli_usage")]
use clap_markdown::{help_markdown_command_custom, MarkdownOptions};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

use crate::context::Context;
use crate::error::IxaError;
use crate::global_properties::load_global_properties_from_map;
use crate::log::level_to_string_list;
use crate::random::ContextRandomExt;
use crate::report::ContextReportExt;
use crate::{info, set_log_level, set_module_filters, LevelFilter};

/// Custom parser for log levels
fn parse_log_levels(s: &str) -> Result<Vec<(String, LevelFilter)>, IxaError> {
    s.split(',')
        .map(|pair| {
            let mut iter = pair.split('=');
            let key = iter.next().ok_or_else(|| IxaError::InvalidLogLevelKey {
                pair: pair.to_string(),
            })?;
            let value = iter.next().ok_or_else(|| IxaError::InvalidLogLevelValue {
                pair: pair.to_string(),
            })?;
            let level = LevelFilter::from_str(value).map_err(|_| IxaError::InvalidLogLevel {
                level: value.to_string(),
            })?;
            Ok((key.to_string(), level))
        })
        .collect()
}

/// Default cli arguments for Ixa runner
#[derive(Args, Debug)]
pub struct BaseArgs {
    #[cfg(feature = "write_cli_usage")]
    /// Print help in Markdown format. This is enabled only for debug builds. Run an example with
    /// `--markdown-help`, and the file `docs/book/src/cli-usage.md` will be written. This file is then
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

| Level   | ERROR | WARN | INFO | DEBUG | TRACE |
|---------|-------|------|------|-------|-------|
| Default |   ✓   |      |      |       |       |
| -v      |   ✓   |  ✓   |  ✓   |       |       |
| -vv     |   ✓   |  ✓   |  ✓   |   ✓   |       |
| -vvv    |   ✓   |  ✓   |  ✓   |   ✓   |   ✓   |
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

    /// Suppresses the printout of summary statistics at the end of the simulation.
    #[arg(long)]
    pub no_stats: bool,
}

impl BaseArgs {
    fn new() -> Self {
        BaseArgs {
            #[cfg(feature = "write_cli_usage")]
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

/// Effective runner arguments after merging defaults, config, and CLI values.
#[derive(Debug)]
pub struct RunnerArgs<A> {
    pub base: BaseArgs,
    pub custom: A,
}

#[derive(Default)]
struct LoadedRunnerConfig {
    args: Option<serde_json::Map<String, serde_json::Value>>,
    global_properties: serde_json::Map<String, serde_json::Value>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PartialBaseArgs {
    random_seed: Option<u64>,
    output_dir: Option<PathBuf>,
    file_prefix: Option<String>,
    force_overwrite: Option<bool>,
    log_level: Option<String>,
    verbose: Option<u8>,
    warn: Option<bool>,
    debug: Option<bool>,
    trace: Option<bool>,
    no_stats: Option<bool>,
}

fn create_ixa_cli() -> Command {
    let cli = Command::new("ixa");
    BaseArgs::augment_args(cli)
}

fn create_ixa_cli_with_custom<A>() -> Command
where
    A: Args,
{
    A::augment_args(create_ixa_cli())
}

fn read_runner_config(config_path: Option<&Path>) -> Result<LoadedRunnerConfig, IxaError> {
    let Some(config_path) = config_path else {
        return Ok(LoadedRunnerConfig::default());
    };

    let config_file = std::fs::File::open(config_path)?;
    let reader = std::io::BufReader::new(config_file);
    let mut config: serde_json::Map<String, serde_json::Value> = serde_json::from_reader(reader)?;
    let args = match config.remove("args") {
        None => None,
        Some(serde_json::Value::Object(args)) => Some(args),
        Some(_) => {
            return Err(IxaError::InvalidRunnerConfig {
                section: "args".to_string(),
                message: "expected a JSON object".to_string(),
            });
        }
    };

    Ok(LoadedRunnerConfig {
        args,
        global_properties: config,
    })
}

fn deserialize_runner_config<T>(value: serde_json::Value, section: &str) -> Result<T, IxaError>
where
    T: DeserializeOwned,
{
    serde_json::from_value(value).map_err(|source| IxaError::InvalidRunnerConfig {
        section: section.to_string(),
        message: source.to_string(),
    })
}

fn arg_was_set_on_command_line(matches: &ArgMatches, id: &str) -> bool {
    matches.value_source(id) == Some(ValueSource::CommandLine)
}

const LOG_ARG_IDS: [&str; 5] = ["log_level", "verbose", "warn", "debug", "trace"];

fn command_line_logging_arg_was_set(matches: &ArgMatches) -> bool {
    LOG_ARG_IDS
        .iter()
        .any(|id| arg_was_set_on_command_line(matches, id))
}

fn reset_logging_args(args: &mut BaseArgs) {
    args.log_level = None;
    args.verbose = 0;
    args.warn = false;
    args.debug = false;
    args.trace = false;
}

fn apply_partial_base_args(args: &mut BaseArgs, partial: PartialBaseArgs) {
    if let Some(random_seed) = partial.random_seed {
        args.random_seed = random_seed;
    }
    if let Some(output_dir) = partial.output_dir {
        args.output_dir = Some(output_dir);
    }
    if let Some(file_prefix) = partial.file_prefix {
        args.file_prefix = Some(file_prefix);
    }
    if let Some(force_overwrite) = partial.force_overwrite {
        args.force_overwrite = force_overwrite;
    }
    if let Some(log_level) = partial.log_level {
        args.log_level = Some(log_level);
    }
    if let Some(verbose) = partial.verbose {
        args.verbose = verbose;
    }
    if let Some(warn) = partial.warn {
        args.warn = warn;
    }
    if let Some(debug) = partial.debug {
        args.debug = debug;
    }
    if let Some(trace) = partial.trace {
        args.trace = trace;
    }
    if let Some(no_stats) = partial.no_stats {
        args.no_stats = no_stats;
    }
}

fn apply_command_line_base_args(args: &mut BaseArgs, cli_args: &BaseArgs, matches: &ArgMatches) {
    if arg_was_set_on_command_line(matches, "random_seed") {
        args.random_seed = cli_args.random_seed;
    }
    if arg_was_set_on_command_line(matches, "output_dir") {
        args.output_dir = cli_args.output_dir.clone();
    }
    if arg_was_set_on_command_line(matches, "file_prefix") {
        args.file_prefix = cli_args.file_prefix.clone();
    }
    if arg_was_set_on_command_line(matches, "force_overwrite") {
        args.force_overwrite = cli_args.force_overwrite;
    }
    if arg_was_set_on_command_line(matches, "log_level") {
        args.log_level = cli_args.log_level.clone();
    }
    if arg_was_set_on_command_line(matches, "verbose") {
        args.verbose = cli_args.verbose;
    }
    if arg_was_set_on_command_line(matches, "warn") {
        args.warn = cli_args.warn;
    }
    if arg_was_set_on_command_line(matches, "debug") {
        args.debug = cli_args.debug;
    }
    if arg_was_set_on_command_line(matches, "trace") {
        args.trace = cli_args.trace;
    }
    if arg_was_set_on_command_line(matches, "no_stats") {
        args.no_stats = cli_args.no_stats;
    }
}

fn merge_base_args(
    matches: &ArgMatches,
    cli_args: &BaseArgs,
    runner_config: Option<&serde_json::Map<String, serde_json::Value>>,
) -> Result<BaseArgs, IxaError> {
    let mut args = BaseArgs::default();

    if let Some(config_object) = runner_config {
        let mut config_object = config_object.clone();
        config_object.remove("custom");
        let partial = deserialize_runner_config(serde_json::Value::Object(config_object), "args")?;
        apply_partial_base_args(&mut args, partial);
    }

    if command_line_logging_arg_was_set(matches) {
        reset_logging_args(&mut args);
    }
    apply_command_line_base_args(&mut args, cli_args, matches);
    args.config = cli_args.config.clone();

    #[cfg(feature = "write_cli_usage")]
    {
        args.markdown_help = cli_args.markdown_help;
    }

    Ok(args)
}

fn serialize_to_object<T>(
    value: &T,
    section: &str,
) -> Result<serde_json::Map<String, serde_json::Value>, IxaError>
where
    T: Serialize,
{
    match serde_json::to_value(value).map_err(|source| IxaError::InvalidRunnerConfig {
        section: section.to_string(),
        message: source.to_string(),
    })? {
        serde_json::Value::Object(object) => Ok(object),
        _ => Err(IxaError::InvalidRunnerConfig {
            section: section.to_string(),
            message: "expected custom args to serialize as a JSON object".to_string(),
        }),
    }
}

fn merge_custom_args<A>(
    matches: &ArgMatches,
    cli_args: &A,
    runner_config: Option<&serde_json::Map<String, serde_json::Value>>,
) -> Result<A, IxaError>
where
    A: Args + Serialize + DeserializeOwned + Default,
{
    let cli_args = serialize_to_object(cli_args, "args.custom")?;
    let mut args = cli_args.clone();
    let mut config_custom_args = None;

    if let Some(config_object) = runner_config {
        if let Some(custom_value) = config_object.get("custom") {
            let serde_json::Value::Object(custom_object) = custom_value else {
                return Err(IxaError::InvalidRunnerConfig {
                    section: "args.custom".to_string(),
                    message: "expected a JSON object".to_string(),
                });
            };
            for (key, value) in custom_object {
                args.insert(key.clone(), value.clone());
            }
            config_custom_args = Some(custom_object);
        }
    }

    for (key, value) in cli_args {
        if arg_was_set_on_command_line(matches, &key) {
            args.insert(key, value);
        }
    }

    validate_required_custom_args::<A>(matches, config_custom_args)?;
    deserialize_runner_config(serde_json::Value::Object(args), "args.custom")
}

fn validate_required_custom_args<A>(
    matches: &ArgMatches,
    config_custom_args: Option<&serde_json::Map<String, serde_json::Value>>,
) -> Result<(), IxaError>
where
    A: Args,
{
    for id in required_custom_arg_ids::<A>() {
        let provided_by_cli = matches.value_source(&id).is_some();
        let provided_by_config = config_custom_args.is_some_and(|custom| custom.contains_key(&id));
        if !provided_by_cli && !provided_by_config {
            return Err(IxaError::InvalidRunnerConfig {
                section: "args.custom".to_string(),
                message: format!(
                    "missing required custom argument `{id}`; provide it on the command line or in args.custom"
                ),
            });
        }
    }

    Ok(())
}

fn required_custom_arg_ids<A>() -> Vec<String>
where
    A: Args,
{
    let base_arg_ids: std::collections::HashSet<_> = create_ixa_cli()
        .get_arguments()
        .map(|arg| arg.get_id().to_string())
        .collect();

    create_ixa_cli_with_custom::<A>()
        .get_arguments()
        .filter(|arg| arg.is_required_set())
        .map(|arg| arg.get_id().to_string())
        .filter(|id| !base_arg_ids.contains(id))
        .collect()
}

fn parse_matches_allowing_config_required_args(
    cli: Command,
    argv: Vec<std::ffi::OsString>,
) -> Result<ArgMatches, clap::Error> {
    match cli.clone().try_get_matches_from(argv.clone()) {
        Ok(matches) => Ok(matches),
        Err(error) if error.kind() == ClapErrorKind::MissingRequiredArgument => cli
            .mut_args(|arg| arg.required(false))
            .try_get_matches_from(argv),
        Err(error) => Err(error),
    }
}

fn custom_args_from_matches<A>(matches: &ArgMatches) -> Result<A, clap::Error>
where
    A: Args + Default,
{
    let mut args = A::default();
    A::update_from_arg_matches(&mut args, matches)?;
    Ok(args)
}

/// Runs a simulation with custom cli arguments.
///
/// This function allows you to define custom arguments and a setup function
///
/// # Parameters
/// - `setup_fn`: A function that takes a mutable reference to a [`Context`], a [`BaseArgs`] struct,
///   a `Option<A>` where `A` is the custom cli arguments struct
///
/// # Errors
/// Returns an error if argument parsing or the setup function fails
pub fn run_with_custom_args<A, F>(setup_fn: F) -> Result<Context, Box<dyn std::error::Error>>
where
    A: Args,
    F: Fn(&mut Context, BaseArgs, Option<A>) -> Result<(), IxaError>,
{
    let cli = create_ixa_cli_with_custom::<A>();
    let matches = cli.get_matches();

    let base_args_matches = BaseArgs::from_arg_matches(&matches)?;
    let custom_matches = A::from_arg_matches(&matches)?;
    let loaded_config = read_runner_config(base_args_matches.config.as_deref())?;
    let effective_base_args =
        merge_base_args(&matches, &base_args_matches, loaded_config.args.as_ref())?;
    execute_runner(effective_base_args, loaded_config, |context, args| {
        setup_fn(context, args, Some(custom_matches))
    })
}

/// Runs a simulation with default cli arguments
///
/// This function parses command line arguments allows you to define a setup function
///
/// # Parameters
/// - `setup_fn`: A function that takes a mutable reference to a [`Context`] and [`BaseArgs`] struct
///
/// # Errors
/// Returns an error if argument parsing or the setup function fails
pub fn run_with_args<F>(setup_fn: F) -> Result<Context, Box<dyn std::error::Error>>
where
    F: Fn(&mut Context, BaseArgs, Option<PlaceholderCustom>) -> Result<(), IxaError>,
{
    let cli = create_ixa_cli();
    let matches = cli.get_matches();

    let base_args_matches = BaseArgs::from_arg_matches(&matches)?;
    let loaded_config = read_runner_config(base_args_matches.config.as_deref())?;
    let effective_base_args =
        merge_base_args(&matches, &base_args_matches, loaded_config.args.as_ref())?;
    execute_runner(effective_base_args, loaded_config, |context, args| {
        setup_fn(context, args, None)
    })
}

/// Runs a simulation with merged base and custom arguments from CLI and config.
///
/// Values in `args` from the JSON config override defaults. Explicit CLI flags
/// override config values. Custom config values are read from `args.custom`.
/// Custom merging supports top-level serde fields whose names match clap arg
/// IDs. Nested or flattened custom layouts are not merged generically.
/// `config` itself is CLI-only and is not read from the config file.
///
/// # Errors
/// Returns an error if argument parsing, config merging, or the setup function fails.
pub fn run_with_merged_args<A, F>(setup_fn: F) -> Result<Context, Box<dyn std::error::Error>>
where
    A: Args + Serialize + DeserializeOwned + Default,
    F: Fn(&mut Context, RunnerArgs<A>) -> Result<(), IxaError>,
{
    let cli = create_ixa_cli_with_custom::<A>();
    let argv: Vec<_> = std::env::args_os().collect();
    let matches = parse_matches_allowing_config_required_args(cli, argv)?;

    let base_args_matches = BaseArgs::from_arg_matches(&matches)?;
    let custom_matches = custom_args_from_matches::<A>(&matches)?;
    let loaded_config = read_runner_config(base_args_matches.config.as_deref())?;
    let effective_base_args =
        merge_base_args(&matches, &base_args_matches, loaded_config.args.as_ref())?;
    let effective_custom_args =
        merge_custom_args(&matches, &custom_matches, loaded_config.args.as_ref())?;

    execute_runner(effective_base_args, loaded_config, |context, base| {
        setup_fn(
            context,
            RunnerArgs {
                base,
                custom: effective_custom_args,
            },
        )
    })
}

#[cfg(test)]
fn run_with_args_internal<A, F>(
    args: BaseArgs,
    custom_args: Option<A>,
    setup_fn: F,
) -> Result<Context, Box<dyn std::error::Error>>
where
    F: Fn(&mut Context, BaseArgs, Option<A>) -> Result<(), IxaError>,
{
    let loaded_config = read_runner_config(args.config.as_deref())?;
    execute_runner(args, loaded_config, |context, args| {
        setup_fn(context, args, custom_args)
    })
}

fn execute_runner<F>(
    args: BaseArgs,
    loaded_config: LoadedRunnerConfig,
    setup_fn: F,
) -> Result<Context, Box<dyn std::error::Error>>
where
    F: FnOnce(&mut Context, BaseArgs) -> Result<(), IxaError>,
{
    #[cfg(feature = "write_cli_usage")]
    // Output help to a markdown file
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
                .join("book")
                .join("src")
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
        load_global_properties_from_map(&mut context, loaded_config.global_properties)?;
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
        } else {
            match parse_log_levels(log_level) {
                Ok(log_levels) => {
                    let log_levels_slice: Vec<(&String, LevelFilter)> =
                        log_levels.iter().map(|(k, v)| (k, *v)).collect();
                    set_module_filters(log_levels_slice.as_slice());
                    for (key, value) in log_levels {
                        println!("Logging enabled for {key} at level {value}");
                        // Here you can set the log level for each key-value pair as needed
                    }
                }
                Err(e) => return Err(Box::new(e)),
            }
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

    if args.no_stats {
        context.print_execution_statistics = false;
    } else {
        if cfg!(target_family = "wasm") {
            info!("the print-stats option is enabled; some statistics are not supported for the wasm target family");
        }
        context.print_execution_statistics = true;
    }

    // Run the provided Fn
    setup_fn(&mut context, args)?;

    // Execute the context
    context.execute();
    Ok(context)
}

#[cfg(test)]
mod tests {
    use std::ffi::OsString;
    use std::fs;

    use serde::{Deserialize, Serialize};
    use serde_json::json;
    use tempfile::tempdir;

    use super::*;
    use crate::global_properties::ContextGlobalPropertiesExt;
    use crate::{define_global_property, define_rng};

    fn fixture_path(name: &str) -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("integration-tests/fixtures/global-properties")
            .join(name)
    }

    #[derive(Args, Debug, Default, Serialize, Deserialize)]
    struct CustomArgs {
        #[arg(short, long, default_value = "0")]
        a: u32,
    }

    #[derive(Args, Debug, Default, Serialize, Deserialize)]
    struct CustomArgsWithClapDefault {
        #[arg(long, default_value_t = 10)]
        count: u32,
    }

    #[derive(Args, Debug, Default, Serialize, Deserialize)]
    struct RequiredCustomArgs {
        #[arg(long)]
        path: PathBuf,
    }

    fn parse_base_args_from<const N: usize>(argv: [&str; N]) -> (ArgMatches, BaseArgs) {
        let matches = create_ixa_cli().try_get_matches_from(argv).unwrap();
        let base_args = BaseArgs::from_arg_matches(&matches).unwrap();
        (matches, base_args)
    }

    fn parse_custom_args_from<A, const N: usize>(argv: [&str; N]) -> (ArgMatches, A)
    where
        A: Args + Default,
    {
        let matches = parse_matches_allowing_config_required_args(
            create_ixa_cli_with_custom::<A>(),
            argv.into_iter().map(OsString::from).collect(),
        )
        .unwrap();
        let custom_args = custom_args_from_matches::<A>(&matches).unwrap();
        (matches, custom_args)
    }

    fn json_object(value: serde_json::Value) -> serde_json::Map<String, serde_json::Value> {
        match value {
            serde_json::Value::Object(object) => object,
            _ => panic!("test value must be a JSON object"),
        }
    }

    #[test]
    fn test_merge_base_args_uses_defaults_without_config() {
        let (matches, cli_args) = parse_base_args_from(["ixa"]);
        let args = merge_base_args(&matches, &cli_args, None).unwrap();

        assert_eq!(args.random_seed, 0);
        assert_eq!(args.output_dir, None);
        assert_eq!(args.file_prefix, None);
        assert!(!args.force_overwrite);
    }

    #[test]
    fn test_merge_base_args_uses_config_values() {
        let (matches, cli_args) = parse_base_args_from(["ixa"]);
        let config = json_object(json!({
            "random_seed": 42,
            "output_dir": "data",
            "file_prefix": "cfg_",
            "force_overwrite": true
        }));
        let args = merge_base_args(&matches, &cli_args, Some(&config)).unwrap();

        assert_eq!(args.random_seed, 42);
        assert_eq!(args.output_dir, Some(PathBuf::from("data")));
        assert_eq!(args.file_prefix, Some("cfg_".to_string()));
        assert!(args.force_overwrite);
    }

    #[test]
    fn test_merge_base_args_cli_overrides_config_values() {
        let (matches, cli_args) = parse_base_args_from([
            "ixa",
            "--random-seed",
            "7",
            "--output",
            "cli-data",
            "--prefix",
            "cli_",
            "--force-overwrite",
        ]);
        let config = json_object(json!({
            "random_seed": 42,
            "output_dir": "data",
            "file_prefix": "cfg_",
            "force_overwrite": false
        }));
        let args = merge_base_args(&matches, &cli_args, Some(&config)).unwrap();

        assert_eq!(args.random_seed, 7);
        assert_eq!(args.output_dir, Some(PathBuf::from("cli-data")));
        assert_eq!(args.file_prefix, Some("cli_".to_string()));
        assert!(args.force_overwrite);
    }

    #[test]
    fn test_merge_base_args_clap_default_does_not_override_config() {
        let (matches, cli_args) = parse_base_args_from(["ixa"]);
        let config = json_object(json!({ "random_seed": 42 }));
        let args = merge_base_args(&matches, &cli_args, Some(&config)).unwrap();

        assert_eq!(args.random_seed, 42);
    }

    #[test]
    fn test_merge_base_args_cli_warn_overrides_config_log_level() {
        let (matches, cli_args) = parse_base_args_from(["ixa", "--warn"]);
        let config = json_object(json!({ "log_level": "trace" }));
        let args = merge_base_args(&matches, &cli_args, Some(&config)).unwrap();

        assert_eq!(args.log_level, None);
        assert_eq!(args.verbose, 0);
        assert!(args.warn);
        assert!(!args.debug);
        assert!(!args.trace);
    }

    #[test]
    fn test_merge_base_args_cli_log_level_overrides_config_verbose() {
        let (matches, cli_args) = parse_base_args_from(["ixa", "--log-level", "error"]);
        let config = json_object(json!({ "verbose": 3 }));
        let args = merge_base_args(&matches, &cli_args, Some(&config)).unwrap();

        assert_eq!(args.log_level, Some("error".to_string()));
        assert_eq!(args.verbose, 0);
        assert!(!args.warn);
        assert!(!args.debug);
        assert!(!args.trace);
    }

    #[test]
    fn test_merge_base_args_rejects_malformed_args_section() {
        let temp_dir = tempdir().unwrap();
        let config_path = temp_dir.path().join("config.json");
        fs::write(&config_path, r#"{ "args": [] }"#).unwrap();

        let err = read_runner_config(Some(&config_path)).err().unwrap();

        assert!(matches!(
            err,
            IxaError::InvalidRunnerConfig { section, .. } if section == "args"
        ));
    }

    #[test]
    fn test_merge_base_args_rejects_unknown_fields() {
        let (matches, cli_args) = parse_base_args_from(["ixa"]);
        let config = json_object(json!({ "random-seed": 42 }));
        let err = merge_base_args(&matches, &cli_args, Some(&config)).unwrap_err();

        assert!(matches!(
            err,
            IxaError::InvalidRunnerConfig { section, .. } if section == "args"
        ));
    }

    #[test]
    fn test_merge_base_args_allows_custom_section() {
        let (matches, cli_args) = parse_base_args_from(["ixa"]);
        let config = json_object(json!({ "custom": { "a": 7 } }));
        let args = merge_base_args(&matches, &cli_args, Some(&config)).unwrap();

        assert_eq!(args.random_seed, 0);
    }

    #[test]
    fn test_merge_custom_args_uses_config_values() {
        let mut cli = create_ixa_cli();
        cli = CustomArgs::augment_args(cli);
        let matches = cli.try_get_matches_from(["ixa"]).unwrap();
        let cli_args = CustomArgs::from_arg_matches(&matches).unwrap();
        let config = json_object(json!({ "custom": { "a": 7 } }));
        let args = merge_custom_args(&matches, &cli_args, Some(&config)).unwrap();

        assert_eq!(args.a, 7);
    }

    #[test]
    fn test_merge_custom_args_cli_overrides_config_values() {
        let mut cli = create_ixa_cli();
        cli = CustomArgs::augment_args(cli);
        let matches = cli.try_get_matches_from(["ixa", "--a", "9"]).unwrap();
        let cli_args = CustomArgs::from_arg_matches(&matches).unwrap();
        let config = json_object(json!({ "custom": { "a": 7 } }));
        let args = merge_custom_args(&matches, &cli_args, Some(&config)).unwrap();

        assert_eq!(args.a, 9);
    }

    #[test]
    fn test_merge_custom_args_preserves_clap_defaults() {
        let (matches, cli_args) = parse_custom_args_from::<CustomArgsWithClapDefault, 1>(["ixa"]);
        let args = merge_custom_args(&matches, &cli_args, None).unwrap();

        assert_eq!(args.count, 10);
    }

    #[test]
    fn test_merge_custom_args_allows_required_args_from_config() {
        let (matches, cli_args) = parse_custom_args_from::<RequiredCustomArgs, 1>(["ixa"]);
        let config = json_object(json!({ "custom": { "path": "from-config.json" } }));
        let args = merge_custom_args(&matches, &cli_args, Some(&config)).unwrap();

        assert_eq!(args.path, PathBuf::from("from-config.json"));
    }

    #[test]
    fn test_merge_custom_args_rejects_missing_required_args() {
        let (matches, cli_args) = parse_custom_args_from::<RequiredCustomArgs, 1>(["ixa"]);
        let err = merge_custom_args(&matches, &cli_args, None).unwrap_err();

        assert!(matches!(
            err,
            IxaError::InvalidRunnerConfig { section, .. } if section == "args.custom"
        ));
    }

    #[test]
    fn test_merge_custom_args_rejects_malformed_custom_section() {
        let mut cli = create_ixa_cli();
        cli = CustomArgs::augment_args(cli);
        let matches = cli.try_get_matches_from(["ixa"]).unwrap();
        let cli_args = CustomArgs::from_arg_matches(&matches).unwrap();
        let config = json_object(json!({ "custom": 7 }));
        let err = merge_custom_args(&matches, &cli_args, Some(&config)).unwrap_err();

        assert!(matches!(
            err,
            IxaError::InvalidRunnerConfig { section, .. } if section == "args.custom"
        ));
    }

    #[test]
    fn test_run_with_custom_args() {
        let result =
            run_with_args_internal(BaseArgs::new(), Some(CustomArgs::default()), |_, _, _| {
                Ok(())
            });
        assert!(result.is_ok());
    }

    #[test]
    fn test_run_with_args() {
        let result = run_with_args_internal(BaseArgs::new(), None, |_, _, _: Option<()>| Ok(()));
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
            config: Some(fixture_path("global_properties_runner.json")),
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
    fn test_run_with_config_path_ignores_args_for_global_properties() {
        let temp_dir = tempdir().unwrap();
        let config_path = temp_dir.path().join("config.json");
        fs::write(
            &config_path,
            r#"{
                        "args": {
                            "random_seed": 42
                            },
                        "ixa.RunnerProperty": {
                            "field_int": 7
                            }
                        }
                    "#,
        )
        .unwrap();

        let test_args = BaseArgs {
            config: Some(config_path),
            ..Default::default()
        };
        let result = run_with_args_internal(test_args, None, |ctx, _, _: Option<()>| {
            let property = ctx.get_global_property_value(RunnerProperty).unwrap();
            assert_eq!(property.field_int, 7);
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
