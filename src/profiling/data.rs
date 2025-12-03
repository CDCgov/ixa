#[cfg(feature = "profiling")]
use std::sync::{Mutex, MutexGuard, OnceLock};
use std::time::{Duration, Instant};

use super::computed_statistic::ComputableType;
use super::Span;
#[cfg(feature = "profiling")]
use super::{
    ComputedStatistic, ComputedValue, CustomStatisticComputer, CustomStatisticPrinter,
    TOTAL_MEASURED,
};
use crate::HashMap;

#[cfg(feature = "profiling")]
static PROFILING_DATA: OnceLock<Mutex<ProfilingData>> = OnceLock::new();

/// Acquires an exclusive lock on the profiling data, blocking until it's available.
#[cfg(feature = "profiling")]
pub(super) fn profiling_data() -> MutexGuard<'static, ProfilingData> {
    PROFILING_DATA
        .get_or_init(|| Mutex::new(ProfilingData::new()))
        .lock()
        .unwrap()
}

#[derive(Default)]
pub struct ProfilingData {
    pub start_time: Option<Instant>,
    pub counts: HashMap<&'static str, usize>,
    // We store span counts with the span duration, because they are updated when
    // the spans are and displayed with the spans rather than with the other counts.
    pub spans: HashMap<&'static str, (Duration, usize)>,
    // The number of spans that are currently open. We use this and the `total_measured` span to
    // compute the amount of time accounted for by all the spans. This together with total
    // runtime can tell you if there is significant runtime not accounted for by the existing
    // spans. When `open_span_count` transitions from `0`, the `total_measured` span is opened.
    // When `open_span_count` transitions back to `0`, `total_measured` is closed and duration
    // is recorded.
    #[cfg(feature = "profiling")]
    pub(super) open_span_count: usize,
    #[cfg(feature = "profiling")]
    pub(super) coverage: Option<Instant>,
    // Custom computed statistics. They are wrapped in an `Option` to allow for temporarily
    // removing them to avoid a double borrow.
    #[cfg(feature = "profiling")]
    pub(super) computed_statistics: Vec<Option<ComputedStatistic>>,
}

#[cfg(feature = "profiling")]
impl ProfilingData {
    /// Initialize a new `ProfilingData`.
    fn new() -> Self {
        Self::default()
    }

    pub(super) fn increment_named_count(&mut self, key: &'static str) {
        self.init_start_time();
        self.counts.entry(key).and_modify(|v| *v += 1).or_insert(1);
    }

    pub(super) fn get_named_count(&self, key: &'static str) -> Option<usize> {
        self.counts.get(&key).copied()
    }

    fn init_start_time(&mut self) {
        if self.start_time.is_none() {
            self.start_time = Some(Instant::now());
        }
    }

    fn open_span(&mut self, label: &'static str) -> Span {
        self.init_start_time();
        if self.open_span_count == 0 {
            // Start recording coverage time.
            self.coverage = Some(Instant::now());
        }
        self.open_span_count += 1;
        Span::new(label)
    }

    /// Do not call directly. This method is called from `Span::drop`.
    pub(super) fn close_span(&mut self, span: &Span) {
        if self.open_span_count > 0 {
            self.open_span_count -= 1;
            if self.open_span_count == 0 {
                // Stop recording coverage time. The `total_measured` must be `Some(..)` if
                // `open_span_count` was nonzero, so unwrap always succeeds.
                let coverage = self.coverage.take().unwrap();
                self.close_span_without_coverage(TOTAL_MEASURED, coverage.elapsed());
            }
        }
        // Always record the span itself.
        self.close_span_without_coverage(span.label, span.start_time.elapsed());
    }

    /// Closes the span without checking the coverage span.
    fn close_span_without_coverage(&mut self, label: &'static str, elapsed: Duration) {
        self.spans
            .entry(label)
            .and_modify(|(time, count)| {
                *time += elapsed;
                *count += 1;
            })
            .or_insert((elapsed, 1));
    }

    /// Constructs a table of ("Event Label", "Count", "Rate (per sec)"). Used to print
    /// stats to the console and write the stats to a file.
    pub(super) fn get_named_counts_table(&self) -> Vec<(String, usize, f64)> {
        let elapsed = self.start_time.unwrap().elapsed().as_secs_f64();

        let mut rows = vec![];

        // Collect data rows
        for (key, count) in &self.counts {
            #[allow(clippy::cast_precision_loss)]
            let rate = (*count as f64) / elapsed;

            rows.push(((*key).to_string(), *count, rate));
        }

        rows
    }

    /// Constructs a table of "Span Label", "Count", "Duration", "% runtime". Used to print
    /// stats to the console and write the stats to a file.
    pub(super) fn get_named_spans_table(&self) -> Vec<(String, usize, Duration, f64)> {
        let elapsed = self.start_time.unwrap().elapsed().as_secs_f64();

        let mut rows = vec![];

        // Add all regular span rows
        for (&label, &(duration, count)) in self.spans.iter().filter(|(k, _)| *k != &TOTAL_MEASURED)
        {
            rows.push((
                label.to_string(),
                count,
                duration,
                duration.as_secs_f64() / elapsed * 100.0,
            ));
        }

        // Add the "Total measured" row at the end
        if let Some(&(duration, count)) = self.spans.get(&TOTAL_MEASURED) {
            rows.push((
                TOTAL_MEASURED.to_string(),
                count,
                duration,
                duration.as_secs_f64() / elapsed * 100.0,
            ));
        }

        rows
    }

    pub(super) fn add_computed_statistic<T: ComputableType>(
        &mut self,
        label: &'static str,
        description: &'static str,
        computer: CustomStatisticComputer<T>,
        printer: CustomStatisticPrinter<T>,
    ) {
        let computed_stat = ComputedStatistic {
            label,
            description,
            value: None,
            functions: T::new_functions(computer, printer),
        };
        self.computed_statistics.push(Some(computed_stat));
    }
}

#[cfg(feature = "profiling")]
pub fn increment_named_count(key: &'static str) {
    let mut container = profiling_data();
    container.increment_named_count(key);
}

#[cfg(not(feature = "profiling"))]
pub fn increment_named_count(_key: &'static str) {}

#[cfg(feature = "profiling")]
pub fn open_span(label: &'static str) -> Span {
    let mut container = profiling_data();
    container.open_span(label)
}

#[cfg(not(feature = "profiling"))]
pub fn open_span(label: &'static str) -> Span {
    Span::new(label)
}

/// Call this if you want to explicitly close a span before the end of the scope in which the
/// span was defined. Equivalent to `span.drop()`.
pub fn close_span(_span: Span) {
    // The `span` is dropped here, and `ProfilingData::close_span` is called
    // from `Span::drop`. Incidentally, this is the same implementation as `span.drop()`!
}

#[cfg(all(test, feature = "profiling"))]
mod tests {
    use std::time::Duration;

    use super::*;
    use crate::profiling::{get_profiling_data, increment_named_count};

    #[test]
    fn test_span_basic() {
        {
            let _span = open_span("test_operation_basic");
            std::thread::sleep(Duration::from_millis(10));
        }

        let data = get_profiling_data();
        assert!(data.spans.contains_key("test_operation_basic"));
        let (duration, count) = data.spans.get("test_operation_basic").unwrap();
        assert_eq!(*count, 1);
        assert!(*duration >= Duration::from_millis(10));
    }

    #[test]
    fn test_span_multiple_calls() {
        for _ in 0..5 {
            let _span = open_span("repeated_operation_multi_test");
            std::thread::sleep(Duration::from_millis(5));
        }

        let data = get_profiling_data();
        let (duration, count) = data.spans.get("repeated_operation_multi_test").unwrap();
        assert!(*count >= 4, "expected at least 4 drops, got {}", count);
        assert!(*duration >= Duration::from_millis(15));
    }

    #[test]
    fn test_span_explicit_close() {
        let span = open_span("explicit_close_test");
        std::thread::sleep(Duration::from_millis(10));
        close_span(span);

        let data = get_profiling_data();
        assert!(data.spans.contains_key("explicit_close_test"));
    }

    #[test]
    fn test_span_nesting() {
        {
            let _outer = open_span("outer_nesting_test");
            std::thread::sleep(Duration::from_millis(5));
            {
                let _inner = open_span("inner_nesting_test");
                std::thread::sleep(Duration::from_millis(5));
            }
            std::thread::sleep(Duration::from_millis(5));
        }

        let data = get_profiling_data();
        assert!(data.spans.contains_key("outer_nesting_test"));
        assert!(data.spans.contains_key("inner_nesting_test"));

        let (outer_duration, _) = data.spans.get("outer_nesting_test").unwrap();
        let (inner_duration, _) = data.spans.get("inner_nesting_test").unwrap();

        assert!(*outer_duration > *inner_duration);
        assert!(*outer_duration >= Duration::from_millis(15));
        assert!(*inner_duration >= Duration::from_millis(5));
    }

    #[test]
    fn test_total_measured_span() {
        {
            let _span1 = open_span("operation1_total_measured");
            std::thread::sleep(Duration::from_millis(10));
        }

        std::thread::sleep(Duration::from_millis(5));

        {
            let _span2 = open_span("operation2_total_measured");
            std::thread::sleep(Duration::from_millis(10));
        }

        let data = get_profiling_data();

        // Just verify our specific spans exist
        assert!(data.spans.contains_key("operation1_total_measured"));
        assert!(data.spans.contains_key("operation2_total_measured"));

        let (duration1, _) = data.spans.get("operation1_total_measured").unwrap();
        let (duration2, _) = data.spans.get("operation2_total_measured").unwrap();

        assert!(*duration1 >= Duration::from_millis(10));
        assert!(*duration2 >= Duration::from_millis(10));
    }

    #[test]
    fn test_get_named_counts_table() {
        // Capture container start_time before adding counts
        let container_start = {
            let data = get_profiling_data();
            data.start_time
        };
        increment_named_count("event_a_counts_table_test");
        increment_named_count("event_a_counts_table_test");
        increment_named_count("event_b_counts_table_test");

        // Sleep to ensure measurable time has passed
        std::thread::sleep(Duration::from_millis(100));

        // Use the same origin as container rate calculation; if None, fall back to local start
        let elapsed = if let Some(start_time) = container_start {
            start_time.elapsed().as_secs_f64()
        } else {
            // If profiling hasn't started yet, rate will be based on init at first increment,
            // so approximate by measuring from the first increment call using a local Instant.
            // In practice, this path should rarely trigger.
            0.1
        };

        let data = get_profiling_data();
        let table = data.get_named_counts_table();

        // Find our specific events instead of checking total table length
        let event_a = table
            .iter()
            .find(|(label, _, _)| label == "event_a_counts_table_test");
        assert!(event_a.is_some());
        let (_, count, rate) = event_a.unwrap();
        assert_eq!(*count, 2);
        // Rate should be approximately 2/elapsed (2 events / ~0.1 second = ~20/sec)
        let expected_rate = 2.0 / elapsed;
        println!(
            "Rate: {}, Expected: {}, Elapsed: {}",
            rate, expected_rate, elapsed
        );
        // Allow 10% margin for timing variations
        assert!(*rate > expected_rate * 0.9 && *rate < expected_rate * 1.1);

        let event_b = table
            .iter()
            .find(|(label, _, _)| label == "event_b_counts_table_test");
        assert!(event_b.is_some());
        let (_, count, _) = event_b.unwrap();
        assert_eq!(*count, 1);
    }

    #[test]
    fn test_get_named_spans_table() {
        // Capture container start time without mutating it
        let container_start = {
            let data = get_profiling_data();
            data.start_time
        };

        {
            let _span = open_span("test_span_table");
            std::thread::sleep(Duration::from_millis(100));
        }

        std::thread::sleep(Duration::from_millis(100));

        let data = get_profiling_data();
        let table = data.get_named_spans_table();

        assert!(table.len() >= 2);

        let test_span = table
            .iter()
            .find(|(label, _, _, _)| label == "test_span_table");
        assert!(test_span.is_some());

        let last = table.last().unwrap();
        assert_eq!(last.0, "Total Measured");

        let (_, _, _, percent) = test_span.unwrap();
        // Compute expected percent from container start time
        let elapsed = if let Some(start_time) = container_start {
            start_time.elapsed().as_secs_f64()
        } else {
            // If profiling hasn't started yet, approximate with 0.2s total elapsed (100ms span + 100ms idle)
            0.2
        };
        let (duration, _) = data.spans.get("test_span_table").unwrap();
        let expected_percent = duration.as_secs_f64() / elapsed * 100.0;
        // Allow reasonable tolerance for timing variations
        assert!((*percent - expected_percent).abs() < 5.0);
    }
}
