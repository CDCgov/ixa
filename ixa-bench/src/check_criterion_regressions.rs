use std::path::{Path, PathBuf};
use std::{env, fs};

use serde_json::Value;
use thiserror::Error;

struct Est {
    group: String,
    bench: String,
    pe: f64,
    lb: f64,
    ub: f64,
}

type TableRow = (String, String, String, String, String);
type NotComparedRow = (String, String, String);

#[derive(Debug)]
struct Args {
    allow_empty: bool,
    baseline: String,
    filter_group: Option<String>,
}

impl Default for Args {
    fn default() -> Self {
        Self {
            allow_empty: false,
            baseline: "base".to_string(),
            filter_group: None,
        }
    }
}

#[derive(Debug)]
struct BenchOutput {
    group: String,
    bench: String,
    dir: PathBuf,
}

struct NotCompared {
    group: String,
    bench: String,
    reason: String,
}

#[derive(Default)]
struct Results {
    regressions: Vec<Est>,
    improvements: Vec<Est>,
    unchanged: Vec<Est>,
    not_compared: Vec<NotCompared>,
}

#[derive(Clone, Copy, Debug)]
enum Verdict {
    Regressed,
    Improved,
    Unchanged,
}

#[derive(Error, Debug)]
enum ReadEstError {
    #[error("read error {path}: {source}")]
    ReadFile {
        path: String,
        #[source]
        source: std::io::Error,
    },
    #[error("json parse {path}: {source}")]
    JsonParse {
        path: String,
        #[source]
        source: serde_json::Error,
    },
    #[error("missing mean")]
    MissingMean,
    #[error("missing point_estimate")]
    MissingPointEstimate,
    #[error("missing confidence_interval")]
    MissingConfidenceInterval,
    #[error("missing lower_bound")]
    MissingLowerBound,
    #[error("missing upper_bound")]
    MissingUpperBound,
}

#[derive(Error, Debug)]
enum ReadVerdictError {
    #[error("read error {path}: {source}")]
    ReadFile {
        path: String,
        #[source]
        source: std::io::Error,
    },
    #[error("could not determine criterion verdict from report {path}")]
    MissingVerdict { path: String },
}

fn parse_args<I, S>(args: I) -> Result<Args, String>
where
    I: IntoIterator<Item = S>,
    S: Into<String>,
{
    let mut parsed = Args::default();
    let mut args = args.into_iter().map(Into::into);

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--allow-empty" => parsed.allow_empty = true,
            "--baseline" => {
                let baseline = args
                    .next()
                    .ok_or_else(|| "--baseline requires a value".to_string())?;
                if baseline.is_empty() {
                    return Err("--baseline requires a non-empty value".to_string());
                }
                parsed.baseline = baseline;
            }
            _ if arg.starts_with("--") => return Err(format!("unknown option: {arg}")),
            _ => {
                if parsed.filter_group.is_some() {
                    return Err(format!("unexpected extra positional argument: {arg}"));
                }
                parsed.filter_group = Some(arg);
            }
        }
    }

    Ok(parsed)
}

fn is_benchmark_output_dir(dir: &Path) -> bool {
    dir.join("change").join("estimates.json").exists()
        || dir.join("new").join("estimates.json").exists()
}

fn find_benchmark_outputs(base: &Path) -> Vec<BenchOutput> {
    let mut results = Vec::new();
    if !base.exists() {
        return results;
    }
    if let Ok(groups) = fs::read_dir(base) {
        for group_entry in groups.flatten() {
            let group_path = group_entry.path();
            if !group_path.is_dir() {
                continue;
            }

            // A directory with criterion outputs directly under it is a benchmark with no group.
            if is_benchmark_output_dir(&group_path) {
                if let Some(bench_name) = group_path.file_name() {
                    results.push(BenchOutput {
                        group: String::new(),
                        bench: bench_name.to_string_lossy().into_owned(),
                        dir: group_path,
                    });
                }
                continue;
            }

            if let Ok(benches) = fs::read_dir(&group_path) {
                for bench_entry in benches.flatten() {
                    let bench_path = bench_entry.path();
                    if !bench_path.is_dir() || !is_benchmark_output_dir(&bench_path) {
                        continue;
                    }

                    if let (Some(group_name), Some(bench_name)) =
                        (group_path.file_name(), bench_path.file_name())
                    {
                        results.push(BenchOutput {
                            group: group_name.to_string_lossy().into_owned(),
                            bench: bench_name.to_string_lossy().into_owned(),
                            dir: bench_path,
                        });
                    }
                }
            }
        }
    }
    results.sort_by(|a, b| a.group.cmp(&b.group).then_with(|| a.bench.cmp(&b.bench)));
    results
}

fn read_est(path: &Path) -> Result<(f64, f64, f64), ReadEstError> {
    let path_str = path.display().to_string();
    let data = fs::read_to_string(path).map_err(|source| ReadEstError::ReadFile {
        path: path_str.clone(),
        source,
    })?;
    let v: Value = serde_json::from_str(&data).map_err(|source| ReadEstError::JsonParse {
        path: path_str.clone(),
        source,
    })?;
    let mean = v.get("mean").ok_or(ReadEstError::MissingMean)?;
    let pe = mean
        .get("point_estimate")
        .and_then(|x| x.as_f64())
        .ok_or(ReadEstError::MissingPointEstimate)?;
    let ci = mean
        .get("confidence_interval")
        .ok_or(ReadEstError::MissingConfidenceInterval)?;
    let lb = ci
        .get("lower_bound")
        .and_then(|x| x.as_f64())
        .ok_or(ReadEstError::MissingLowerBound)?;
    let ub = ci
        .get("upper_bound")
        .and_then(|x| x.as_f64())
        .ok_or(ReadEstError::MissingUpperBound)?;
    Ok((pe, lb, ub))
}

fn read_verdict(path: &Path) -> Result<Verdict, ReadVerdictError> {
    let path_str = path.display().to_string();
    let html = fs::read_to_string(path).map_err(|source| ReadVerdictError::ReadFile {
        path: path_str.clone(),
        source,
    })?;

    if html.contains("Performance has regressed.") {
        return Ok(Verdict::Regressed);
    }
    if html.contains("Performance has improved.") {
        return Ok(Verdict::Improved);
    }
    if html.contains("Change within noise threshold.")
        || html.contains("No change in performance detected.")
    {
        return Ok(Verdict::Unchanged);
    }

    Err(ReadVerdictError::MissingVerdict { path: path_str })
}

fn print_table(
    title: &str,
    rows: &[(String, String, String, String, String)],
    widths: &[usize; 5],
) {
    if rows.is_empty() {
        println!("{}: (none)", title);
        return;
    }

    println!("{}:", title);
    println!(
        "  {}  {}  {}  {}  {}",
        "Group".pad_to_width(widths[0]),
        "Bench".pad_to_width(widths[1]),
        "Change".pad_left_to_width(widths[2]),
        "CI Lower".pad_left_to_width(widths[3]),
        "CI Upper".pad_left_to_width(widths[4]),
    );
    println!(
        "  {}  {}  {}  {}  {}",
        "-".repeat(widths[0]),
        "-".repeat(widths[1]),
        "-".repeat(widths[2]),
        "-".repeat(widths[3]),
        "-".repeat(widths[4])
    );
    for r in rows {
        println!(
            "  {}  {}  {}  {}  {}",
            r.0.pad_to_width(widths[0]),
            r.1.pad_to_width(widths[1]),
            r.2.pad_left_to_width(widths[2]),
            r.3.pad_left_to_width(widths[3]),
            r.4.pad_left_to_width(widths[4])
        );
    }
    println!();
}

fn print_not_compared_table(title: &str, rows: &[NotComparedRow], widths: &[usize; 3]) {
    if rows.is_empty() {
        println!("{}: (none)", title);
        return;
    }

    println!("{}:", title);
    println!(
        "  {}  {}  {}",
        "Group".pad_to_width(widths[0]),
        "Bench".pad_to_width(widths[1]),
        "Reason".pad_to_width(widths[2]),
    );
    println!(
        "  {}  {}  {}",
        "-".repeat(widths[0]),
        "-".repeat(widths[1]),
        "-".repeat(widths[2])
    );
    for row in rows {
        println!(
            "  {}  {}  {}",
            row.0.pad_to_width(widths[0]),
            row.1.pad_to_width(widths[1]),
            row.2.pad_to_width(widths[2])
        );
    }
    println!();
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

fn collect_results(base: &Path, args: &Args) -> Result<Results, String> {
    let outputs = find_benchmark_outputs(base);
    if outputs.is_empty() && !args.allow_empty {
        return Err(format!(
            "No criterion outputs found under {}",
            base.display()
        ));
    }

    let mut results = Results::default();

    for output in outputs {
        if let Some(ref filter_group) = args.filter_group {
            if filter_group != &output.group {
                continue;
            }
        }

        let change_path = output.dir.join("change").join("estimates.json");
        if change_path.exists() {
            let (pe, lb, ub) = read_est(&change_path)
                .map_err(|err| format!("Error parsing {}: {err}", change_path.display()))?;
            let report_path = output.dir.join("report").join("index.html");
            let verdict = read_verdict(&report_path)
                .map_err(|err| format!("Error parsing {}: {err}", report_path.display()))?;

            let est = Est {
                group: output.group,
                bench: output.bench,
                pe,
                lb,
                ub,
            };
            match verdict {
                Verdict::Regressed => results.regressions.push(est),
                Verdict::Improved => results.improvements.push(est),
                Verdict::Unchanged => results.unchanged.push(est),
            }
        } else if output.dir.join("new").join("estimates.json").exists() {
            let baseline_path = output.dir.join(&args.baseline).join("estimates.json");
            let reason = if baseline_path.exists() {
                "No comparison output generated".to_string()
            } else {
                format!("No baseline named '{}'", args.baseline)
            };
            results.not_compared.push(NotCompared {
                group: output.group,
                bench: output.bench,
                reason,
            });
        }
    }

    Ok(results)
}

fn display_group(group: &str) -> String {
    if group.is_empty() {
        "(no group)".to_string()
    } else {
        group.to_string()
    }
}

fn print_results(results: &Results) {
    let format_pct = |v: f64| format!("{:.3}%", v * 100.0);
    let reg_rows: Vec<TableRow> = results
        .regressions
        .iter()
        .map(|e| {
            (
                display_group(&e.group),
                e.bench.clone(),
                format_pct(e.pe),
                format_pct(e.lb),
                format_pct(e.ub),
            )
        })
        .collect();
    let imp_rows: Vec<TableRow> = results
        .improvements
        .iter()
        .map(|e| {
            (
                display_group(&e.group),
                e.bench.clone(),
                format_pct(e.pe),
                format_pct(e.lb),
                format_pct(e.ub),
            )
        })
        .collect();
    let un_rows: Vec<TableRow> = results
        .unchanged
        .iter()
        .map(|e| {
            (
                display_group(&e.group),
                e.bench.clone(),
                format_pct(e.pe),
                format_pct(e.lb),
                format_pct(e.ub),
            )
        })
        .collect();
    let not_compared_rows: Vec<NotComparedRow> = results
        .not_compared
        .iter()
        .map(|e| (display_group(&e.group), e.bench.clone(), e.reason.clone()))
        .collect();

    let widths = compute_global_widths(&[&reg_rows, &imp_rows, &un_rows]);
    print_table("Regressions", &reg_rows, &widths);
    print_table("Improvements", &imp_rows, &widths);
    print_table("Unchanged", &un_rows, &widths);

    let not_compared_widths = compute_not_compared_widths(&not_compared_rows);
    print_not_compared_table("Not Compared", &not_compared_rows, &not_compared_widths);
}

fn main() {
    let args = match parse_args(env::args().skip(1)) {
        Ok(args) => args,
        Err(err) => {
            eprintln!("{err}");
            eprintln!(
                "Usage: check_criterion_regressions [--allow-empty] [--baseline NAME] [group]"
            );
            std::process::exit(2);
        }
    };

    match collect_results(Path::new("target/criterion"), &args) {
        Ok(results) => print_results(&results),
        Err(err) => {
            eprintln!("{err}");
            std::process::exit(1);
        }
    }
}

fn compute_global_widths(tables: &[&[TableRow]]) -> [usize; 5] {
    let mut widths = [
        "Group".len(),
        "Bench".len(),
        "Change".len(),
        "CI Lower".len(),
        "CI Upper".len(),
    ];

    for table in tables {
        for row in *table {
            widths[0] = widths[0].max(row.0.len());
            widths[1] = widths[1].max(row.1.len());
            widths[2] = widths[2].max(row.2.len());
            widths[3] = widths[3].max(row.3.len());
            widths[4] = widths[4].max(row.4.len());
        }
    }

    widths
}

fn compute_not_compared_widths(rows: &[NotComparedRow]) -> [usize; 3] {
    let mut widths = ["Group".len(), "Bench".len(), "Reason".len()];

    for row in rows {
        widths[0] = widths[0].max(row.0.len());
        widths[1] = widths[1].max(row.1.len());
        widths[2] = widths[2].max(row.2.len());
    }

    widths
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::Path;

    use tempfile::tempdir;

    use super::{collect_results, parse_args, Args};

    fn write_estimates(path: &Path, point_estimate: f64, lower: f64, upper: f64) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(
            path,
            format!(
                r#"{{
  "mean": {{
    "point_estimate": {point_estimate},
    "confidence_interval": {{
      "lower_bound": {lower},
      "upper_bound": {upper}
    }}
  }}
}}"#
            ),
        )
        .unwrap();
    }

    fn write_report(path: &Path, verdict: &str) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, verdict).unwrap();
    }

    #[test]
    fn parse_args_accepts_flags_and_optional_group() {
        let args = parse_args(["--allow-empty", "--baseline", "main", "sampling"]).unwrap();

        assert!(args.allow_empty);
        assert_eq!(args.baseline, "main");
        assert_eq!(args.filter_group.as_deref(), Some("sampling"));
    }

    #[test]
    fn collect_results_reads_change_outputs() {
        let dir = tempdir().unwrap();
        let bench_dir = dir.path().join("sampling").join("sample_one");
        write_estimates(
            &bench_dir.join("change").join("estimates.json"),
            0.1,
            0.05,
            0.15,
        );
        write_report(
            &bench_dir.join("report").join("index.html"),
            "Performance has regressed.",
        );

        let results = collect_results(dir.path(), &Args::default()).unwrap();

        assert_eq!(results.regressions.len(), 1);
        assert_eq!(results.regressions[0].group, "sampling");
        assert_eq!(results.regressions[0].bench, "sample_one");
        assert_eq!(results.regressions[0].pe, 0.1);
        assert!(results.improvements.is_empty());
        assert!(results.unchanged.is_empty());
        assert!(results.not_compared.is_empty());
    }

    #[test]
    fn collect_results_reports_new_benchmarks_without_baseline() {
        let dir = tempdir().unwrap();
        let bench_dir = dir.path().join("sampling").join("new_sample");
        write_estimates(
            &bench_dir.join("new").join("estimates.json"),
            123.0,
            100.0,
            140.0,
        );

        let results = collect_results(dir.path(), &Args::default()).unwrap();

        assert_eq!(results.not_compared.len(), 1);
        assert_eq!(results.not_compared[0].group, "sampling");
        assert_eq!(results.not_compared[0].bench, "new_sample");
        assert_eq!(results.not_compared[0].reason, "No baseline named 'base'");
        assert!(results.regressions.is_empty());
    }

    #[test]
    fn collect_results_empty_requires_allow_empty() {
        let dir = tempdir().unwrap();

        assert!(collect_results(dir.path(), &Args::default()).is_err());

        let args = Args {
            allow_empty: true,
            ..Args::default()
        };
        let results = collect_results(dir.path(), &args).unwrap();
        assert!(results.regressions.is_empty());
        assert!(results.improvements.is_empty());
        assert!(results.unchanged.is_empty());
        assert!(results.not_compared.is_empty());
    }
}
