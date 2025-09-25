use ixa::{HashSet, HashSetExt};
use serde_json::Value;
use std::env;
use std::fs;
use std::path::Path;
use std::time::{Duration, SystemTime};

struct Est {
    group: String,
    bench: String,
    pe: f64,
    lb: f64,
    ub: f64,
}

fn find_change_files(base: &Path) -> Vec<(String, String, std::path::PathBuf)> {
    let mut results = Vec::new();
    if !base.exists() {
        return results;
    }
    if let Ok(groups) = fs::read_dir(base) {
        for g in groups.flatten() {
            let gpath = g.path();
            if !gpath.is_dir() {
                continue;
            }
            if let Ok(benches) = fs::read_dir(&gpath) {
                for b in benches.flatten() {
                    let bpath = b.path();
                    if !bpath.is_dir() {
                        continue;
                    }
                    let change_file = bpath.join("change").join("estimates.json");
                    if change_file.exists() {
                        if let (Some(gs), Some(bs)) = (gpath.file_name(), bpath.file_name()) {
                            results.push((
                                gs.to_string_lossy().into_owned(),
                                bs.to_string_lossy().into_owned(),
                                change_file,
                            ));
                        }
                    }
                }
            }
        }
    }
    results
}

fn read_est(path: &Path) -> Result<(f64, f64, f64), String> {
    let data =
        fs::read_to_string(path).map_err(|e| format!("read error {}: {}", path.display(), e))?;
    let v: Value =
        serde_json::from_str(&data).map_err(|e| format!("json parse {}: {}", path.display(), e))?;
    let mean = v.get("mean").ok_or_else(|| "missing mean".to_string())?;
    let pe = mean
        .get("point_estimate")
        .and_then(|x| x.as_f64())
        .ok_or_else(|| "missing point_estimate".to_string())?;
    let ci = mean
        .get("confidence_interval")
        .ok_or_else(|| "missing confidence_interval".to_string())?;
    let lb = ci
        .get("lower_bound")
        .and_then(|x| x.as_f64())
        .ok_or_else(|| "missing lower_bound".to_string())?;
    let ub = ci
        .get("upper_bound")
        .and_then(|x| x.as_f64())
        .ok_or_else(|| "missing upper_bound".to_string())?;
    Ok((pe, lb, ub))
}

fn is_recent(path: &Path, recent_seconds: u64) -> bool {
    match fs::metadata(path).and_then(|m| m.modified()) {
        Ok(mtime) => match SystemTime::now().duration_since(mtime) {
            Ok(dur) => dur <= Duration::from_secs(recent_seconds),
            Err(_) => false,
        },
        Err(_) => false,
    }
}

fn print_table(title: &str, rows: &[(String, String, String, String, String)]) {
    if rows.is_empty() {
        println!("{}: (none)", title);
        return;
    }
    // headers
    let headers = ("Group", "Bench", "Change", "CI Lower", "CI Upper");
    // compute widths
    let mut cols: Vec<Vec<String>> = vec![
        vec![headers.0.to_string()],
        vec![headers.1.to_string()],
        vec![headers.2.to_string()],
        vec![headers.3.to_string()],
        vec![headers.4.to_string()],
    ];
    for r in rows {
        cols[0].push(r.0.clone());
        cols[1].push(r.1.clone());
        cols[2].push(r.2.clone());
        cols[3].push(r.3.clone());
        cols[4].push(r.4.clone());
    }
    let widths: Vec<usize> = cols
        .iter()
        .map(|c| c.iter().map(|s| s.len()).max().unwrap_or(0))
        .collect();
    println!("{}:", title);
    // header line
    println!(
        "  {}  {}  {}  {}  {}",
        headers.0.pad_to_width(widths[0]),
        headers.1.pad_to_width(widths[1]),
        headers.2.pad_to_width(widths[2]),
        headers.3.pad_to_width(widths[3]),
        headers.4.pad_to_width(widths[4])
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
            r.2.pad_to_width(widths[2]),
            r.3.pad_to_width(widths[3]),
            r.4.pad_to_width(widths[4])
        );
    }
    println!();
}

trait Pad {
    fn pad_to_width(&self, w: usize) -> String;
}

impl Pad for &str {
    fn pad_to_width(&self, w: usize) -> String {
        let mut s = self.to_string();
        if s.len() < w {
            s.push_str(&" ".repeat(w - s.len()));
        }
        s
    }
}

impl Pad for String {
    fn pad_to_width(&self, w: usize) -> String {
        self.as_str().pad_to_width(w)
    }
}

fn main() {
    let args: Vec<String> = env::args().collect();
    let filter_group = if args.len() > 1 {
        Some(args[1].clone())
    } else {
        None
    };
    let base = Path::new("target/criterion");
    let change_files = find_change_files(base);
    if change_files.is_empty() {
        eprintln!("No criterion change outputs found under target/criterion");
        std::process::exit(1);
    }

    // detect recent groups if no explicit filter
    let mut recent_groups: Option<HashSet<String>> = None;
    if filter_group.is_none() {
        let mut set: HashSet<String> = HashSet::new();
        for entry in change_files.iter() {
            let g = &entry.0;
            let gpath = base.join(g);
            if is_recent(&gpath, 300) {
                set.insert(g.clone());
                continue;
            }
            // check benches
            let bpath = base.join(g).join(&entry.1);
            if is_recent(&bpath, 300) {
                set.insert(g.clone());
            }
        }
        if !set.is_empty() {
            recent_groups = Some(set);
        }
    }

    let mut regressions: Vec<Est> = Vec::new();
    let mut improvements: Vec<Est> = Vec::new();
    let mut unchanged: Vec<Est> = Vec::new();

    for (group, bench, path) in change_files {
        // filtering
        if let Some(ref fg) = filter_group {
            if fg != &group {
                continue;
            }
        } else if let Some(ref rg) = recent_groups {
            if !rg.contains(&group) {
                continue;
            }
        }
        match read_est(&path) {
            Ok((pe, lb, ub)) => {
                let e = Est {
                    group: group.clone(),
                    bench: bench.clone(),
                    pe,
                    lb,
                    ub,
                };
                if pe > 0.0 && lb > 0.0 {
                    regressions.push(e);
                } else if pe < 0.0 && ub < 0.0 {
                    improvements.push(e);
                } else {
                    unchanged.push(e);
                }
            }
            Err(err) => {
                eprintln!("Error parsing {}: {}", path.display(), err);
                std::process::exit(1);
            }
        }
    }

    // Prepare rows
    let format_pct = |v: f64| format!("{:.3}%", v * 100.0);
    let reg_rows: Vec<_> = regressions
        .iter()
        .map(|e| {
            (
                e.group.clone(),
                e.bench.clone(),
                format_pct(e.pe),
                format_pct(e.lb),
                format_pct(e.ub),
            )
        })
        .collect();
    let imp_rows: Vec<_> = improvements
        .iter()
        .map(|e| {
            (
                e.group.clone(),
                e.bench.clone(),
                format_pct(e.pe),
                format_pct(e.lb),
                format_pct(e.ub),
            )
        })
        .collect();
    let un_rows: Vec<_> = unchanged
        .iter()
        .map(|e| {
            (
                e.group.clone(),
                e.bench.clone(),
                format_pct(e.pe),
                format_pct(e.lb),
                format_pct(e.ub),
            )
        })
        .collect();

    // Print three tables
    print_table("Regressions", &reg_rows);
    print_table("Improvements", &imp_rows);
    print_table("Unchanged", &un_rows);

    std::process::exit(0);
}
