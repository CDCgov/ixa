use std::fs::{self, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

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
    let groups = match args.group {
        Some(group) => vec![group],
        None => list_groups()
            .into_iter()
            .map(|group| group.to_string())
            .collect(),
    };

    if groups.len() > 1 {
        if let Some(export_spec) = find_markdown_export(&extra_args) {
            run_groups_with_markdown_export(&groups, &extra_args, export_spec);
            return;
        }
    }

    for group in groups {
        run_hyperfine(&group, &extra_args);
    }
}

fn run_hyperfine(group: &str, extra_args: &[String]) {
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

struct ExportSpec {
    base_path: PathBuf,
    arg_index: usize,
    uses_equals: bool,
}

impl ExportSpec {
    fn apply(&self, args: &mut [String], new_path: &Path) {
        let path_string = new_path.to_string_lossy().into_owned();
        if self.uses_equals {
            args[self.arg_index] = format!("--export-markdown={path_string}");
        } else {
            args[self.arg_index] = path_string;
        }
    }
}

fn find_markdown_export(args: &[String]) -> Option<ExportSpec> {
    args.iter().enumerate().find_map(|(idx, arg)| {
        if arg == "--export-markdown" {
            Some(ExportSpec {
                base_path: PathBuf::from(args.get(idx + 1)?),
                arg_index: idx + 1,
                uses_equals: false,
            })
        } else {
            arg.strip_prefix("--export-markdown=")
                .map(|path| ExportSpec {
                    base_path: PathBuf::from(path),
                    arg_index: idx,
                    uses_equals: true,
                })
        }
    })
}

fn run_groups_with_markdown_export(
    groups: &[String],
    extra_args: &[String],
    export_spec: ExportSpec,
) {
    if let Some(parent) = export_spec.base_path.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent)
                .unwrap_or_else(|err| panic!("Failed to create export directory: {err}"));
        }
    }
    if export_spec.base_path.exists() {
        fs::remove_file(&export_spec.base_path).unwrap_or_else(|err| {
            panic!(
                "Failed to remove existing export file {}: {err}",
                export_spec.base_path.display()
            )
        });
    }

    for (idx, group) in groups.iter().enumerate() {
        let mut args_for_group = extra_args.to_vec();
        let group_path = group_export_path(&export_spec.base_path, group);
        export_spec.apply(&mut args_for_group[..], &group_path);

        run_hyperfine(group, &args_for_group);

        append_markdown(&export_spec.base_path, &group_path, idx == 0)
            .unwrap_or_else(|err| panic!("Failed to append markdown export: {err}"));

        let _ = fs::remove_file(&group_path);
    }
}

fn group_export_path(base: &Path, group: &str) -> PathBuf {
    let file_name = base
        .file_stem()
        .and_then(|stem| stem.to_str())
        .map(|stem| format!("{stem}_{}", sanitize_group_name(group)))
        .unwrap_or_else(|| format!("hyperfine_{}", sanitize_group_name(group)));

    let mut path = base.with_file_name(file_name);
    if let Some(ext) = base.extension() {
        path.set_extension(ext);
    }
    path
}

fn sanitize_group_name(group: &str) -> String {
    group
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect()
}

fn append_markdown(
    aggregate_path: &Path,
    group_path: &Path,
    is_first: bool,
) -> std::io::Result<()> {
    let content = fs::read_to_string(group_path)?;

    if is_first {
        // Write the first table, ensuring it ends with a newline but not multiple
        let trimmed = content.trim_end();
        let mut file = fs::File::create(aggregate_path)?;
        file.write_all(trimmed.as_bytes())?;
        file.write_all(b"\n")?;
    } else {
        // Parse the new content to extract data rows only (skip header and separator)
        let new_rows: Vec<&str> = content
            .lines()
            .skip(2) // Skip header and separator line
            .filter(|line| !line.trim().is_empty())
            .collect();

        // Append new rows to existing content
        let mut file = OpenOptions::new().append(true).open(aggregate_path)?;

        for row in new_rows {
            file.write_all(row.as_bytes())?;
            file.write_all(b"\n")?;
        }
    }
    Ok(())
}
