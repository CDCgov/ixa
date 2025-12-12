use std::fs::{self, File};
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

    let mut all_rows = Vec::new();
    for group in groups {
        let mut args_for_group = extra_args.to_vec();
        let group_path = group_export_path(&export_spec.base_path, group);
        export_spec.apply(&mut args_for_group[..], &group_path);

        run_hyperfine(group, &args_for_group);

        let content = fs::read_to_string(&group_path)
            .unwrap_or_else(|err| panic!("Failed to read markdown export: {err}"));
        all_rows.extend(parse_hyperfine_rows(&content));

        let _ = fs::remove_file(&group_path);
    }

    write_hyperfine_table(&all_rows, &export_spec.base_path)
        .unwrap_or_else(|err| panic!("Failed to write hyperfine table: {err}"));
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

fn parse_hyperfine_rows(content: &str) -> Vec<HyperfineRow> {
    content
        .lines()
        .skip(2)
        .filter_map(|line| {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                return None;
            }
            let parts: Vec<&str> = line
                .split('|')
                .map(str::trim)
                .filter(|part| !part.is_empty())
                .collect();
            if parts.len() != 5 {
                return None;
            }
            let command = parts[0].trim_matches('`');
            let (group, bench) = if let Some(pos) = command.find("::") {
                (command[..pos].to_string(), command[pos + 2..].to_string())
            } else {
                (command.to_string(), "".to_string())
            };
            Some(HyperfineRow {
                group,
                bench,
                mean: parts[1].to_string(),
                min: parts[2].to_string(),
                max: parts[3].to_string(),
                relative: parts[4].to_string(),
            })
        })
        .collect()
}

fn write_hyperfine_table(rows: &[HyperfineRow], path: &Path) -> std::io::Result<()> {
    let mut file = File::create(path)?;
    if rows.is_empty() {
        writeln!(file, "  (none)")?;
        return Ok(());
    }

    let headers = (
        "Group",
        "Bench",
        "Mean [ms]",
        "Min [ms]",
        "Max [ms]",
        "Relative",
    );
    let mut cols: Vec<Vec<String>> = vec![
        vec![headers.0.to_string()],
        vec![headers.1.to_string()],
        vec![headers.2.to_string()],
        vec![headers.3.to_string()],
        vec![headers.4.to_string()],
        vec![headers.5.to_string()],
    ];
    for row in rows {
        cols[0].push(row.group.clone());
        cols[1].push(row.bench.clone());
        cols[2].push(row.mean.clone());
        cols[3].push(row.min.clone());
        cols[4].push(row.max.clone());
        cols[5].push(row.relative.clone());
    }
    let widths: Vec<usize> = cols
        .iter()
        .map(|c| c.iter().map(|s| s.len()).max().unwrap_or(0))
        .collect();

    writeln!(
        file,
        "  {}  {}  {}  {}  {}  {}",
        headers.0.pad_to_width(widths[0]),
        headers.1.pad_to_width(widths[1]),
        headers.2.pad_left_to_width(widths[2]),
        headers.3.pad_left_to_width(widths[3]),
        headers.4.pad_left_to_width(widths[4]),
        headers.5.pad_left_to_width(widths[5])
    )?;
    writeln!(
        file,
        "  {}  {}  {}  {}  {}  {}",
        "-".repeat(widths[0]),
        "-".repeat(widths[1]),
        "-".repeat(widths[2]),
        "-".repeat(widths[3]),
        "-".repeat(widths[4]),
        "-".repeat(widths[5])
    )?;
    for row in rows {
        writeln!(
            file,
            "  {}  {}  {}  {}  {}  {}",
            row.group.pad_to_width(widths[0]),
            row.bench.pad_to_width(widths[1]),
            row.mean.pad_left_to_width(widths[2]),
            row.min.pad_left_to_width(widths[3]),
            row.max.pad_left_to_width(widths[4]),
            row.relative.pad_left_to_width(widths[5])
        )?;
    }
    Ok(())
}

struct HyperfineRow {
    group: String,
    bench: String,
    mean: String,
    min: String,
    max: String,
    relative: String,
}

trait Pad {
    fn pad_to_width(&self, w: usize) -> String;
    fn pad_left_to_width(&self, w: usize) -> String;
}

impl Pad for &str {
    fn pad_to_width(&self, w: usize) -> String {
        let mut s = self.to_string();
        if s.len() < w {
            s.push_str(&" ".repeat(w - s.len()));
        }
        s
    }

    fn pad_left_to_width(&self, w: usize) -> String {
        let s = self.to_string();
        if s.len() < w {
            format!("{}{}", " ".repeat(w - s.len()), s)
        } else {
            s
        }
    }
}

impl Pad for String {
    fn pad_to_width(&self, w: usize) -> String {
        self.as_str().pad_to_width(w)
    }

    fn pad_left_to_width(&self, w: usize) -> String {
        self.as_str().pad_left_to_width(w)
    }
}
