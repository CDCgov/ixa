// Loss of precision is allowable in this module's use cases.
#![allow(clippy::cast_precision_loss)]

use std::time::Duration;
#[cfg(not(target_arch = "wasm32"))]
use std::time::Instant;

use bytesize::ByteSize;
use humantime::format_duration;
use log::{debug, error, info};
use serde_derive::Serialize;
use sysinfo::{Pid, ProcessRefreshKind, ProcessesToUpdate, System};
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;
#[cfg(target_arch = "wasm32")]
use web_sys::window;

#[cfg_attr(target_arch = "wasm32", allow(dead_code))]
/// How frequently we update the max memory used value.
const REFRESH_INTERVAL: Duration = Duration::from_secs(1);

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
/// The `wasm` target does not support `std::time::Instant::now()`.
pub fn get_high_res_time() -> f64 {
    let perf = window().unwrap().performance().unwrap();
    perf.now() // Returns time in milliseconds as f64
}

/// A container struct for computed final statistics. Note that if population size
/// is zero, then the per person statistics are also zero, as they are meaningless.
#[derive(Serialize)]
pub struct ExecutionStatistics {
    pub max_memory_usage: u64,
    pub cpu_time: Duration,
    pub wall_time: Duration,

    // Per person stats
    pub population: usize,
    pub cpu_time_per_person: Duration,
    pub wall_time_per_person: Duration,
    pub memory_per_person: u64,
}

#[cfg_attr(target_arch = "wasm32", allow(dead_code))]
pub(crate) struct ExecutionProfilingCollector {
    /// Simulation start time, used to compute elapsed wall time for the simulation execution
    #[cfg(not(target_arch = "wasm32"))]
    start_time: Instant,
    #[cfg(target_arch = "wasm32")]
    start_time: f64,
    /// We keep track of the last time we refreshed so that client code doesn't have to and can
    /// just call `ExecutionProfilingCollector::refresh` in its event loop.
    #[cfg(not(target_arch = "wasm32"))]
    last_refresh: Instant,
    #[cfg(target_arch = "wasm32")]
    last_refresh: f64,
    /// The accumulated CPU time of the process in CPU-milliseconds at simulation start, used
    /// to compute the CPU time of the simulation execution
    start_cpu_time: u64,
    /// The maximum amount of real memory used by the process as reported by
    /// `sysinfo::System::process::memory()`. This value is polled during execution to capture the
    /// max.
    max_memory_usage: u64,
    /// A `sysinfo::System` for polling memory use
    system: System,
    /// Current process, set to `None` on unsupported platforms, wasm32 in particular
    process_id: Option<Pid>,
}

impl ExecutionProfilingCollector {
    pub fn new() -> ExecutionProfilingCollector {
        let process_id = sysinfo::get_current_pid().ok();
        #[cfg(target_arch = "wasm32")]
        let now = get_high_res_time();
        #[cfg(not(target_arch = "wasm32"))]
        let now = Instant::now();

        let mut new_stats = ExecutionProfilingCollector {
            start_time: now,
            last_refresh: now,
            start_cpu_time: 0,
            max_memory_usage: 0,
            system: System::new(),
            process_id,
        };
        // Only refreshable on supported platforms.
        if let Some(process_id) = process_id {
            debug!("Process ID: {}", process_id);
            let process_refresh_kind = ProcessRefreshKind::nothing().with_cpu().with_memory();
            new_stats.update_system_info(process_refresh_kind);

            let process = new_stats.system.process(process_id).unwrap();

            new_stats.max_memory_usage = process.memory();
            new_stats.start_cpu_time = process.accumulated_cpu_time();
        }

        new_stats
    }

    /// If at least `REFRESH_INTERVAL` (1 second) has passed since the previous
    /// refresh, memory usage is polled and updated. Call this method as frequently
    /// as you like, as it takes care of limiting polling frequency itself.
    #[inline]
    pub fn refresh(&mut self) {
        #[cfg(not(target_arch = "wasm32"))]
        if self.last_refresh.elapsed() >= REFRESH_INTERVAL {
            self.poll_memory();
            self.last_refresh = Instant::now();
        }
    }

    #[cfg_attr(target_arch = "wasm32", allow(dead_code))]
    /// Updates maximum memory usage. This method should be called about once per second,
    /// as it is a relatively expensive system call.
    fn poll_memory(&mut self) {
        if let Some(pid) = self.process_id {
            // Only refreshes memory statistics
            self.update_system_info(ProcessRefreshKind::nothing().with_memory());
            let process = self.system.process(pid).unwrap();
            self.max_memory_usage = self.max_memory_usage.max(process.memory());
        }
    }

    /// Gives accumulated CPU time of the process in CPU-milliseconds since simulation start.
    #[allow(unused)]
    pub fn cpu_time(&mut self) -> u64 {
        if let Some(process_id) = self.process_id {
            // Only refresh cpu statistics
            self.update_system_info(ProcessRefreshKind::nothing().with_cpu());

            let process = self.system.process(process_id).unwrap();
            process.accumulated_cpu_time() - self.start_cpu_time
        } else {
            0
        }
    }

    /// Refreshes the internal `sysinfo::System` object for this process using the given
    /// `ProcessRefreshKind`.
    #[inline]
    fn update_system_info(&mut self, process_refresh_kind: ProcessRefreshKind) {
        if let Some(pid) = self.process_id {
            if self.system.refresh_processes_specifics(
                ProcessesToUpdate::Some(&[pid]),
                true,
                process_refresh_kind,
            ) < 1
            {
                error!("could not refresh process statistics");
            }
        }
    }

    /// Computes the final summary statistics
    pub fn compute_final_statistics(&mut self, population: usize) -> ExecutionStatistics {
        let mut cpu_time_millis = 0;

        if let Some(pid) = self.process_id {
            // Update both memory and cpu statistics
            self.update_system_info(ProcessRefreshKind::nothing().with_cpu().with_memory());
            let process = self.system.process(pid).unwrap();

            self.max_memory_usage = self.max_memory_usage.max(process.memory());
            cpu_time_millis = process.accumulated_cpu_time() - self.start_cpu_time;
        }

        // Convert to `Duration`s in preparation for formatting
        let cpu_time = Duration::from_millis(cpu_time_millis);
        #[cfg(target_arch = "wasm32")]
        let wall_time = get_high_res_time() - self.start_time;
        #[cfg(not(target_arch = "wasm32"))]
        let wall_time = self.start_time.elapsed();

        // For the per person stats, it's not clear what scale this should be at. Duration can
        // be constructed from seconds in `f64`, which is probably good enough for our purposes.
        // For memory, we can just round to the nearest byte.

        let cpu_time_per_person = if population > 0 {
            Duration::from_secs_f64(cpu_time_millis as f64 / population as f64 / 1000.0)
        } else {
            Duration::new(0, 0)
        };
        let wall_time_per_person = if population > 0 {
            #[cfg(not(target_arch = "wasm32"))]
            let wall_time = wall_time.as_secs_f64();
            #[cfg(target_arch = "wasm32")]
            let wall_time = wall_time / 1000.0;
            Duration::from_secs_f64(wall_time / population as f64)
        } else {
            Duration::new(0, 0)
        };
        let memory_per_person = if population > 0 {
            self.max_memory_usage / population as u64
        } else {
            0
        };

        #[cfg(target_arch = "wasm32")]
        let wall_time = Duration::from_millis(wall_time as u64);

        ExecutionStatistics {
            max_memory_usage: self.max_memory_usage,
            cpu_time,
            wall_time,

            // Per person stats
            population,
            cpu_time_per_person,
            wall_time_per_person,
            memory_per_person,
        }
    }
}

/// Prints execution statistics to the console.
///
/// Use `ExecutionProfilingCollector::compute_final_statistics()` to construct `ExecutionStatistics`.
pub fn print_execution_statistics(summary: &ExecutionStatistics) {
    println!("━━━━ Execution Summary ━━━━");
    if summary.max_memory_usage == 0 {
        println!("Memory and CPU statistics are not available on your platform.");
    } else {
        println!(
            "{:<25}{}",
            "Max memory usage:",
            ByteSize::b(summary.max_memory_usage)
        );
        println!("{:<25}{}", "CPU time:", format_duration(summary.cpu_time));
    }

    println!("{:<25}{}", "Wall time:", format_duration(summary.wall_time));

    if summary.population > 0 {
        println!("{:<25}{}", "Population:", summary.population);
        if summary.max_memory_usage > 0 {
            println!(
                "{:<25}{}",
                "Memory per person:",
                ByteSize::b(summary.memory_per_person)
            );
            println!(
                "{:<25}{}",
                "CPU time per person:",
                format_duration(summary.cpu_time_per_person)
            );
        }
        println!(
            "{:<25}{}",
            "Wall time per person:",
            format_duration(summary.wall_time_per_person)
        );
    }
}

/// Logs execution statistics with the logging system.
///
/// Use `ExecutionProfilingCollector::compute_final_statistics()` to construct `ExecutionStatistics`.
pub fn log_execution_statistics(stats: &ExecutionStatistics) {
    info!("Execution complete.");
    if stats.max_memory_usage == 0 {
        info!("Memory and CPU statistics are not available on your platform.");
    } else {
        info!("Max memory usage: {}", ByteSize::b(stats.max_memory_usage));
        info!("CPU time: {}", format_duration(stats.cpu_time));
    }
    info!("Wall time: {}", format_duration(stats.wall_time));

    if stats.population > 0 {
        info!("Population: {}", stats.population);
        if stats.max_memory_usage > 0 {
            info!(
                "Memory per person: {}",
                ByteSize::b(stats.memory_per_person)
            );
            info!(
                "CPU time per person: {}",
                format_duration(stats.cpu_time_per_person)
            );
        }
        info!(
            "Wall time per person: {}",
            format_duration(stats.wall_time_per_person)
        );
    }
}

#[cfg(test)]
mod tests {
    use std::thread;
    use std::time::Duration;

    use super::*;

    #[test]
    fn test_collector_initialization() {
        let collector = ExecutionProfilingCollector::new();

        // Ensure that initial max memory usage is non-zero
        assert!(collector.max_memory_usage > 0);
    }

    #[test]
    fn test_refresh_respects_interval() {
        let mut collector = ExecutionProfilingCollector::new();
        let before = collector.max_memory_usage;

        // Call refresh immediately — it should not poll
        collector.refresh();
        let after = collector.max_memory_usage;
        assert_eq!(before, after);

        // Sleep enough time to trigger refresh
        thread::sleep(Duration::from_secs(2));
        collector.refresh();
        // Now memory usage should be refreshed — allow it to stay same or increase
        assert!(collector.max_memory_usage >= before);
    }

    #[test]
    fn test_compute_final_statistics_structure() {
        let mut collector = ExecutionProfilingCollector::new();

        thread::sleep(Duration::from_millis(100));
        let stats = collector.compute_final_statistics(10);

        // Fields should be non-zero
        assert!(stats.max_memory_usage > 0);
        assert!(stats.wall_time > Duration::ZERO);
        assert_eq!(stats.population, 10);
    }

    #[test]
    fn test_zero_population_results() {
        let mut collector = ExecutionProfilingCollector::new();

        let stats = collector.compute_final_statistics(0);

        assert_eq!(stats.population, 0);
        assert_eq!(stats.cpu_time_per_person, Duration::ZERO);
        assert_eq!(stats.wall_time_per_person, Duration::ZERO);
        assert_eq!(stats.memory_per_person, 0);
    }

    #[test]
    fn test_cpu_time_increases_over_time() {
        let mut collector = ExecutionProfilingCollector::new();

        // Burn ~30ms CPU time. Likely will be < 30ms, as this thread will not have 100% of CPU
        // during 30ms wall time.
        let start = Instant::now();
        while start.elapsed().as_millis() < 30u128 {
            std::hint::black_box(0); // Prevent optimization
        }

        let cpu_time_1 = collector.cpu_time();

        // Burn ~50ms CPU time
        let start = Instant::now();
        while start.elapsed().as_millis() < 50u128 {
            std::hint::black_box(0); // Prevent optimization
        }

        let cpu_time_2 = collector.cpu_time();
        assert!(cpu_time_2 > cpu_time_1);
    }
}
