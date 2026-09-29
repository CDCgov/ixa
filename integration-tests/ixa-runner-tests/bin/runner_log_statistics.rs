use std::time::Duration;

use ixa::execution_stats::{log_execution_statistics, ExecutionStatistics};
use ixa::runner::run_with_args;

fn main() {
    run_with_args(|_context, _args, _| {
        log_execution_statistics(&ExecutionStatistics {
            max_memory_usage: 0,
            max_plans_in_flight: 0,
            max_plan_queue_memory_in_use: 0,
            cpu_time: Duration::ZERO,
            wall_time: Duration::from_secs(1),
        });
        Ok(())
    })
    .unwrap();
}
