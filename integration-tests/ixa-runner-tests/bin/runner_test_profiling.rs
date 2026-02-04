use std::time::Duration;

use ixa::profiling::{
    add_computed_statistic, increment_named_count, open_span, ProfilingContextExt,
};
use ixa::runner::run_with_args;

fn main() {
    let mut context = run_with_args(|context, _args, _| {
        context.add_plan(0.0, |context| {
            increment_named_count("it_prof_event");
            increment_named_count("it_prof_event");
            increment_named_count("it_prof_event");

            {
                let _span = open_span("it_prof_span");
                // Ensure elapsed runtime is nonzero so derived rates/percentages are well-defined.
                std::thread::sleep(Duration::from_millis(10));
            }

            add_computed_statistic::<usize>(
                "it_prof_stat",
                "Total test events",
                Box::new(|data| data.counts.get("it_prof_event").copied()),
                Box::new(|_value| {}),
            );

            context.shutdown();
        });
        Ok(())
    })
    .unwrap();

    context.write_profiling_data();
}
