use super::registry;
use clap::{builder::PossibleValuesParser, Parser, Subcommand};

#[derive(Subcommand)]
enum BenchRunnerCommand {
    List {
        #[arg(short, long, value_parser= PossibleValuesParser::new(registry::list_groups()))]
        group: Option<String>,

        #[arg(long)]
        one_line: bool,
    },
    Run {
        #[arg(short, long,  value_parser= PossibleValuesParser::new(registry::list_groups()))]
        group: String,

        #[arg(short, long)]
        bench: String,
    },
}

#[derive(Parser)]
#[command(version, about, long_about = None)]
struct BenchRunnerArgs {
    #[command(subcommand)]
    command: BenchRunnerCommand,
}

pub struct BenchRunner;

impl BenchRunner {
    pub fn run_from_args(&self) {
        let args = BenchRunnerArgs::parse();
        // TODO: validate the bench name against the group
        self.run_command(args.command);
    }

    fn run_command(&self, command: BenchRunnerCommand) {
        match command {
            BenchRunnerCommand::List { group, one_line } => {
                let output = if let Some(g) = group {
                    registry::list_benches(&g).unwrap()
                } else {
                    registry::list_groups()
                };
                // Formatting
                if one_line {
                    print!("{}", output.join(","));
                } else {
                    for line in output {
                        println!("{}", line);
                    }
                }
            }
            BenchRunnerCommand::Run { group, bench } => {
                if let Err(e) = registry::run_bench(&group, &bench) {
                    eprintln!("Error running benchmark: {}", e);
                    std::process::exit(1);
                }
            }
        }
    }
}
