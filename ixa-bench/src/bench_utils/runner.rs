use clap::{builder::PossibleValuesParser, Parser};
use ixa_bench::bench_utils::registry;

fn main() {
    let args = BenchRunnerArgs::parse();
    if let Err(e) = registry::run_bench(&args.group, &args.bench) {
        eprintln!("Error running benchmark: {}", e);
        std::process::exit(1);
    }
}

#[derive(Parser)]
struct BenchRunnerArgs {
    #[arg(short, long, value_parser= PossibleValuesParser::new(registry::list_groups()))]
    group: String,
    #[arg(short, long)]
    bench: String,
}
