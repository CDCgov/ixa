use std::io::{BufRead, BufReader};
use std::process::{Command, Stdio};
use std::vec;

use clap::builder::PossibleValuesParser;
use clap::{arg, Parser};
use ixa_bench::bench_utils::registry::{list_benches, list_groups};

const STYLE_FLAGS: [&str; 2] = ["--show-output", "--style"];

#[derive(Parser)]
struct HyperfineArgs {
    #[arg(value_parser = PossibleValuesParser::new(list_groups()))]
    /// Name of the benchmark group(s) to run.
    group: Option<String>,
    #[arg(long, default_value = "3")]
    warmup: String,
    #[arg(long, default_value = "50")]
    runs: String,
    #[arg(last = true, trailing_var_arg = true, allow_hyphen_values = true)]
    /// Extra args to pass to hyperfine
    extra: Vec<String>,
}
impl HyperfineArgs {
    fn as_arg_string(&self) -> Vec<String> {
        let mut hyperfine_args = vec![
            "--warmup".into(),
            self.warmup.clone(),
            "--runs".into(),
            self.runs.clone(),
        ];
        hyperfine_args.extend(self.extra.clone());
        hyperfine_args
    }
}

fn main() {
    let args = HyperfineArgs::parse();
    let extra_args = args.as_arg_string();

    if let Some(group) = args.group {
        run_hyperfine(&group, &extra_args);
    } else {
        for group in list_groups() {
            run_hyperfine(group, &extra_args);
        }
    }
}

fn run_hyperfine(group: &str, extra_args: &Vec<String>) {
    let benches = list_benches(group).expect("Failed to get benches for group");
    let mut cmd = Command::new("hyperfine");
    if !extra_args.iter().any(|a| STYLE_FLAGS.contains(&a.as_str())) {
        cmd.args(["--style", "color"]);
    }
    cmd.arg("-N")
        .args(extra_args)
        .args(["--parameter-list", "bench_id", benches.join(",").as_str()])
        .args(["--command-name", &format!("{group}::{{bench_id}}")])
        .arg(format!(
            "./target/release/run_bench --group {group} --bench {{bench_id}}"
        ))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    println!("\x1b[2m{:?}\x1b[0m", cmd);

    let mut child = cmd.spawn().expect("failed to execute hyperfine");

    if let Some(stdout) = child.stdout.take() {
        let reader = BufReader::new(stdout);
        let stdout_thread = std::thread::spawn(move || {
            for line in reader.lines().map_while(Result::ok) {
                println!("{}", line);
            }
        });
        if let Some(stderr) = child.stderr.take() {
            let reader = BufReader::new(stderr);
            let stderr_thread = std::thread::spawn(move || {
                for line in reader.lines().map_while(Result::ok) {
                    eprintln!("{}", line);
                }
            });
            stderr_thread.join().unwrap();
        }

        stdout_thread.join().unwrap();
    }
    let status = child.wait().expect("failed to wait on hyperfine");
    if !status.success() {
        eprintln!("Hyperfine failed for group {}", group);
        std::process::exit(1);
    }
}
