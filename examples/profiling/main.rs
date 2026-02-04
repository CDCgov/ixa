use std::path::PathBuf;

use ixa::prelude::*;
use ixa::profiling::{
    add_computed_statistic, increment_named_count, open_span, print_profiling_data,
    ProfilingContextExt,
};

fn main() {
    let mut context = Context::new();

    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let output_dir = if manifest_dir.ends_with("examples/profiling") {
        // Running as the standalone `ixa_example_profiling` package.
        manifest_dir.join("output")
    } else {
        // Running as an example of the workspace root crate.
        manifest_dir
            .join("examples")
            .join("profiling")
            .join("output")
    };

    std::fs::create_dir_all(&output_dir).unwrap_or_else(|e| {
        panic!(
            "failed to create output directory {}: {e}",
            output_dir.display()
        )
    });

    context
        .report_options()
        .directory(&output_dir)
        .file_prefix("example_")
        .overwrite(true);

    context.add_plan(0.0, |context| {
        increment_named_count("example_profiling:event");
        increment_named_count("example_profiling:event");
        increment_named_count("example_profiling:event");

        {
            let _span = open_span("example_profiling:span");
            // Do a small amount of deterministic work (no sleep), and ensure it isn't optimized away.
            let mut acc: u64 = 0;
            for i in 0..50_000u64 {
                acc = acc.wrapping_add(i.wrapping_mul(31));
                std::hint::black_box(acc);
            }
        }

        add_computed_statistic::<usize>(
            "example_profiling:stat",
            "Total example events",
            Box::new(|data| data.counts.get("example_profiling:event").copied()),
            Box::new(|value| println!("Computed stat example_profiling:stat = {value}")),
        );

        context.shutdown();
    });

    context.execute();

    print_profiling_data();

    // This will write to: <output_dir>/<file_prefix>profiling.json
    context.write_profiling_data();

    let profiling_path = output_dir.join("example_profiling.json");
    println!("Profiling JSON path: {}", profiling_path.display());

    if profiling_path.exists() {
        let content = std::fs::read_to_string(&profiling_path)
            .unwrap_or_else(|e| panic!("failed to read {}: {e}", profiling_path.display()));
        serde_json::from_str::<serde_json::Value>(&content)
            .unwrap_or_else(|e| panic!("profiling output was not valid JSON: {e}"));
    } else {
        println!(
            "Profiling JSON was not created (profiling may be disabled): {}",
            profiling_path.display()
        );
    }
}
