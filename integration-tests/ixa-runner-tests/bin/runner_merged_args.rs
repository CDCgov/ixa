use clap::Args;
use ixa::runner::run_with_merged_args;
use ixa::RunnerArgs;
use serde::{Deserialize, Serialize};

#[derive(Args, Debug, Default, Serialize, Deserialize)]
struct Extra {
    #[arg(short, long, default_value = "0")]
    a: u32,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    run_with_merged_args(|_context, args: RunnerArgs<Extra>| {
        println!("seed={} a={}", args.base.random_seed, args.custom.a);
        Ok(())
    })?;
    Ok(())
}
