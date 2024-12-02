use std::path::{Path, PathBuf};

use crate::context::Context;
use crate::error::IxaError;
use crate::global_properties::ContextGlobalPropertiesExt;
use crate::random::ContextRandomExt;
use crate::report::ContextReportExt;
use clap::Parser;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
pub struct Args {
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

#[allow(clippy::missing_errors_doc)]
pub fn run_with_args<F>(load: F) -> Result<(), Box<dyn std::error::Error>>
where
    F: Fn(&mut Context, Args) -> Result<(), IxaError>,
{
    run_with_args_internal(Args::parse(), load)
}

fn run_with_args_internal<F>(args: Args, load: F) -> Result<(), Box<dyn std::error::Error>>
where
    F: Fn(&mut Context, Args) -> Result<(), IxaError>,
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

    context.init_random(args.seed);

    // Run the provided Fn
    load(&mut context, args)?;

    // Execute the context
    context.execute();
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::define_global_property;
    use serde::Deserialize;

    #[test]
    fn test_run_with_args_default() {
        let result = run_with_args(|_, _| Ok(()));
        assert!(result.is_ok());
    }

    #[test]
    fn test_run_with_random_seed() {
        let test_args = Args {
            seed: 42,
            config: String::new(),
            output_dir: String::new(),
        };
        let result = run_with_args_internal(test_args, |ctx, _| {
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
        let test_args = Args {
            seed: 42,
            config: "tests/data/global_properties_runner.json".to_string(),
            output_dir: String::new(),
        };
        let result = run_with_args_internal(test_args, |ctx, _| {
            let p3 = ctx.get_global_property_value(RunnerProperty).unwrap();
            assert_eq!(p3.field_int, 0);
            Ok(())
        });
        assert!(result.is_ok());
    }

    #[test]
    fn test_run_with_output_dir() {
        let test_args = Args {
            seed: 42,
            config: String::new(),
            output_dir: "data".to_string(),
        };
        let result = run_with_args_internal(test_args, |ctx, _| {
            let output_dir = &ctx.report_options().directory;
            assert_eq!(output_dir, &PathBuf::from("data"));
            Ok(())
        });
        assert!(result.is_ok());
    }
}
