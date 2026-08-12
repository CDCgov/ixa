#[cfg(feature = "profiling")]
use std::cell::RefCell;
#[cfg(feature = "profiling")]
use std::collections::hash_map::Entry;
#[cfg(feature = "profiling")]
use std::ptr::eq;
#[cfg(feature = "profiling")]
use std::sync::{Mutex, MutexGuard, OnceLock};
use std::time::Duration;

use super::computed_statistic::ComputableType;
#[cfg(feature = "profiling")]
use super::ProfilingInstant;
use super::Span;
#[cfg(feature = "profiling")]
use super::{
    ComputedStatistic, ComputedValue, CustomStatisticComputer, CustomStatisticPrinter,
    TOTAL_MEASURED,
};
#[cfg(feature = "profiling")]
use crate::entity::multi_property::{query_identity_label, QueryIdentityId};
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
    #[cfg(feature = "profiling")]
    pub start_time: Option<ProfilingInstant>,
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
    pub(super) coverage: Option<ProfilingInstant>,
    // Custom computed statistics. They are wrapped in an `Option` to allow for temporarily
    // removing them to avoid a double borrow.
    #[cfg(feature = "profiling")]
    pub(super) computed_statistics: Vec<Option<ComputedStatistic>>,
}

/// Aggregate profiling data for all committed executions of one query shape.
///
/// [`QueryProfiler`] stores one value per query identity and updates it
/// whenever a completed query execution is recorded.
#[cfg(feature = "profiling")]
#[derive(Clone, Copy, Debug)]
pub(crate) struct QueryProfilingData {
    pub(crate) count: usize,
    pub(crate) total: Duration,
    pub(crate) min: Duration,
    pub(crate) max: Duration,
}

#[cfg(feature = "profiling")]
impl QueryProfilingData {
    fn new(elapsed: Duration) -> Self {
        Self {
            count: 1,
            total: elapsed,
            min: elapsed,
            max: elapsed,
        }
    }

    fn record(&mut self, elapsed: Duration) {
        self.count += 1;
        self.total += elapsed;
        self.min = self.min.min(elapsed);
        self.max = self.max.max(elapsed);
    }
}

/// Context-owned storage for aggregate query-profiling data.
///
/// Recording handles submit completed query identities and elapsed durations to
/// this collection. Interior mutability permits profiling through query APIs
/// that borrow the owning context immutably.
#[cfg(feature = "profiling")]
#[derive(Default)]
pub(crate) struct QueryProfiler {
    timings: RefCell<HashMap<QueryIdentityId, QueryProfilingData>>,
}

#[cfg(feature = "profiling")]
impl QueryProfiler {
    fn record(&self, identity: QueryIdentityId, elapsed: Duration) {
        let mut timings = self.timings.borrow_mut();
        match timings.entry(identity) {
            Entry::Occupied(mut entry) => entry.get_mut().record(elapsed),
            Entry::Vacant(entry) => {
                entry.insert(QueryProfilingData::new(elapsed));
            }
        }
    }

    /// Returns `(query label, aggregate profiling data)` pairs sorted by
    /// descending total duration and then ascending query label.
    pub(crate) fn snapshot(&self) -> Vec<(&'static str, QueryProfilingData)> {
        let timings = self.timings.borrow();
        let mut rows = timings
            .iter()
            .map(|(&identity, &data)| (query_identity_label(identity), data))
            .collect::<Vec<_>>();

        rows.sort_by(|(left_query, left_data), (right_query, right_data)| {
            right_data
                .total
                .cmp(&left_data.total)
                .then_with(|| left_query.cmp(right_query))
        });
        rows
    }

    #[cfg(test)]
    pub(crate) fn query_profiling_data(
        &self,
        identity: QueryIdentityId,
    ) -> Option<QueryProfilingData> {
        self.timings.borrow().get(&identity).copied()
    }
}

/// Copyable recording destination for one query shape in one context.
///
/// Carrying this handle does not record an execution. Contiguous timing scopes
/// and discontinuous execution profiles use it to commit completed elapsed
/// durations to the owning [`QueryProfiler`].
#[cfg(feature = "profiling")]
#[derive(Clone, Copy)]
pub(crate) struct QueryProfileHandle<'a> {
    profiler: &'a QueryProfiler,
    identity: QueryIdentityId,
}

#[cfg(feature = "profiling")]
impl<'a> QueryProfileHandle<'a> {
    pub(crate) fn new(profiler: &'a QueryProfiler, identity: QueryIdentityId) -> Self {
        Self { profiler, identity }
    }

    fn record(self, elapsed: Duration) {
        self.profiler.record(self.identity, elapsed);
    }

    #[must_use]
    pub(crate) fn scope(self) -> QueryProfileScope<'a> {
        QueryProfileScope {
            handle: self,
            start_time: ProfilingInstant::now(),
        }
    }

    pub(crate) fn execution(self) -> QueryExecutionProfile<'a> {
        QueryExecutionProfile::new(self)
    }
}

#[cfg(feature = "profiling")]
impl PartialEq for QueryProfileHandle<'_> {
    fn eq(&self, other: &Self) -> bool {
        eq(self.profiler, other.profiler) && self.identity == other.identity
    }
}

#[cfg(feature = "profiling")]
impl Eq for QueryProfileHandle<'_> {}

/// RAII guard that records one continuously measured query execution.
///
/// The guard starts timing when constructed and commits one elapsed duration
/// through its [`QueryProfileHandle`] when dropped.
#[cfg(feature = "profiling")]
pub(crate) struct QueryProfileScope<'a> {
    handle: QueryProfileHandle<'a>,
    start_time: ProfilingInstant,
}

#[cfg(feature = "profiling")]
impl Drop for QueryProfileScope<'_> {
    fn drop(&mut self) {
        self.handle.record(self.start_time.elapsed());
    }
}

/// Accumulates discontinuous query work for one lazy iterator execution.
///
/// Each [`QueryExecutionScope`] adds one elapsed work slice. [`Self::finish`]
/// or `Drop` commits the accumulated duration once; an execution that never
/// opened a work scope records nothing.
#[cfg(feature = "profiling")]
pub(crate) struct QueryExecutionProfile<'a> {
    handle: Option<QueryProfileHandle<'a>>,
    elapsed: Option<Duration>,
}

#[cfg(feature = "profiling")]
impl<'a> QueryExecutionProfile<'a> {
    fn new(handle: QueryProfileHandle<'a>) -> Self {
        Self {
            handle: Some(handle),
            elapsed: None,
        }
    }

    #[must_use]
    pub(crate) fn scope(&mut self) -> Option<QueryExecutionScope<'_, 'a>> {
        self.handle.as_ref()?;
        self.elapsed.get_or_insert(Duration::ZERO);
        Some(QueryExecutionScope {
            execution: self,
            start_time: ProfilingInstant::now(),
        })
    }

    pub(crate) fn finish(&mut self) {
        if let (Some(handle), Some(elapsed)) = (self.handle.take(), self.elapsed.take()) {
            handle.record(elapsed);
        }
    }
}

#[cfg(feature = "profiling")]
impl Drop for QueryExecutionProfile<'_> {
    fn drop(&mut self) {
        self.finish();
    }
}

/// RAII guard for one timed work slice in a discontinuous query execution.
///
/// Dropping the guard adds the slice's elapsed duration to its
/// [`QueryExecutionProfile`]. It does not commit an aggregate observation.
#[cfg(feature = "profiling")]
pub(crate) struct QueryExecutionScope<'execution, 'context> {
    execution: &'execution mut QueryExecutionProfile<'context>,
    start_time: ProfilingInstant,
}

#[cfg(feature = "profiling")]
impl Drop for QueryExecutionScope<'_, '_> {
    fn drop(&mut self) {
        *self.execution.elapsed.get_or_insert(Duration::ZERO) += self.start_time.elapsed();
    }
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
            self.start_time = Some(ProfilingInstant::now());
        }
    }

    fn open_span(&mut self, label: &'static str) -> Span {
        self.init_start_time();
        if self.open_span_count == 0 {
            // Start recording coverage time.
            self.coverage = Some(ProfilingInstant::now());
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
        let elapsed = match self.start_time {
            Some(start_time) => start_time.elapsed().as_secs_f64(),
            None => 0.0,
        };
        let mut rows = vec![];

        // Collect data rows
        for (key, count) in &self.counts {
            let rate = (*count as f64) / elapsed; // Just allow this to be `inf`/`nan` if `elapsed == 0.0`.

            rows.push(((*key).to_string(), *count, rate));
        }

        rows
    }

    /// Constructs a table of "Span Label", "Count", "Duration", "% runtime". Used to print
    /// stats to the console and write the stats to a file.
    pub(super) fn get_named_spans_table(&self) -> Vec<(String, usize, Duration, f64)> {
        let elapsed = match self.start_time {
            Some(start_time) => start_time.elapsed().as_secs_f64(),
            None => 0.0,
        };

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
#[must_use]
pub fn open_span(label: &'static str) -> Span {
    let mut container = profiling_data();
    container.open_span(label)
}

#[cfg(not(feature = "profiling"))]
#[must_use]
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
    use crate::entity::multi_property::test_query_identity;
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
            // so approximate the elapsed time from the first increment call.
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

    #[test]
    fn query_profiler_first_observation_initializes_record() {
        let profiler = QueryProfiler::default();
        let identity = test_query_identity("QueryTimingInit: (Age)");
        profiler.record(identity, Duration::from_micros(10));

        let data = profiler.query_profiling_data(identity).unwrap();
        assert_eq!(data.count, 1);
        assert_eq!(data.total, Duration::from_micros(10));
        assert_eq!(data.min, Duration::from_micros(10));
        assert_eq!(data.max, Duration::from_micros(10));
    }

    #[test]
    fn query_profiler_later_observations_update_aggregate() {
        let profiler = QueryProfiler::default();
        let identity = test_query_identity("QueryTimingUpdate: (Age)");
        profiler.record(identity, Duration::from_micros(10));
        profiler.record(identity, Duration::from_micros(30));
        profiler.record(identity, Duration::from_micros(5));

        let data = profiler.query_profiling_data(identity).unwrap();
        assert_eq!(data.count, 3);
        assert_eq!(data.total, Duration::from_micros(45));
        assert_eq!(data.min, Duration::from_micros(5));
        assert_eq!(data.max, Duration::from_micros(30));
    }

    #[test]
    fn query_profiler_snapshot_includes_count_total_min_and_max() {
        let profiler = QueryProfiler::default();
        let identity = test_query_identity("QueryTimingTable: (Age)");
        profiler.record(identity, Duration::from_millis(10));
        profiler.record(identity, Duration::from_millis(30));

        let snapshot = profiler.snapshot();
        let (_, data) = snapshot
            .iter()
            .find(|(query, _)| *query == "QueryTimingTable: (Age)")
            .unwrap();

        assert_eq!(data.count, 2);
        assert_eq!(data.total, Duration::from_millis(40));
        assert_eq!(data.min, Duration::from_millis(10));
        assert_eq!(data.max, Duration::from_millis(30));
    }

    #[test]
    fn query_profiler_same_identity_aggregates_into_one_row() {
        let profiler = QueryProfiler::default();
        let identity = test_query_identity("QueryTimingSame: (Age)");
        profiler.record(identity, Duration::from_micros(10));
        profiler.record(identity, Duration::from_micros(30));
        profiler.record(identity, Duration::from_micros(5));

        let data = profiler.query_profiling_data(identity).unwrap();
        assert_eq!(data.count, 3);
        assert_eq!(data.total, Duration::from_micros(45));
    }

    #[test]
    fn query_profiler_snapshot_sorts_by_total_then_label() {
        let profiler = QueryProfiler::default();
        let b = test_query_identity("QueryTimingSort: B");
        let c = test_query_identity("QueryTimingSort: C");
        let a = test_query_identity("QueryTimingSort: A");
        profiler.record(b, Duration::from_micros(20));
        profiler.record(c, Duration::from_micros(10));
        profiler.record(a, Duration::from_micros(20));

        let snapshot = profiler.snapshot();
        let labels = snapshot.iter().map(|(query, _)| *query).collect::<Vec<_>>();

        assert_eq!(
            labels,
            vec![
                "QueryTimingSort: A",
                "QueryTimingSort: B",
                "QueryTimingSort: C"
            ]
        );
    }

    #[test]
    fn query_profile_scope_records_on_drop() {
        let profiler = QueryProfiler::default();
        let identity = test_query_identity("QueryProfileScope: (Age)");
        {
            let _scope = QueryProfileHandle::new(&profiler, identity).scope();
        }

        let data = profiler.query_profiling_data(identity).unwrap();
        assert_eq!(data.count, 1);
    }

    #[test]
    fn query_profile_handle_equality_uses_profiler_and_identity() {
        fn requires_eq<T: Eq>(_value: T) {}

        let first_profiler = QueryProfiler::default();
        let second_profiler = QueryProfiler::default();
        let first_identity = test_query_identity("QueryHandleEquality: A");
        let second_identity = test_query_identity("QueryHandleEquality: B");

        let first = QueryProfileHandle::new(&first_profiler, first_identity);
        let same = QueryProfileHandle::new(&first_profiler, first_identity);
        let different_identity = QueryProfileHandle::new(&first_profiler, second_identity);
        let different_profiler = QueryProfileHandle::new(&second_profiler, first_identity);

        assert!(first == same);
        assert!(first != different_identity);
        assert!(first != different_profiler);
        requires_eq(first);
    }

    #[test]
    fn query_execution_scopes_accumulate_into_one_observation() {
        let profiler = QueryProfiler::default();
        let identity = test_query_identity("QueryExecutionScopes: (Age)");
        {
            let mut execution = QueryProfileHandle::new(&profiler, identity).execution();
            drop(execution.scope());
            drop(execution.scope());
        }

        let data = profiler.query_profiling_data(identity).unwrap();
        assert_eq!(data.count, 1);
    }

    #[test]
    fn unused_query_execution_records_nothing() {
        let profiler = QueryProfiler::default();
        let identity = test_query_identity("UnusedQueryExecution: (Age)");
        drop(QueryProfileHandle::new(&profiler, identity).execution());

        assert!(profiler.query_profiling_data(identity).is_none());
    }

    #[test]
    fn explicitly_finished_query_execution_records_only_once() {
        let profiler = QueryProfiler::default();
        let identity = test_query_identity("FinishedQueryExecution: (Age)");
        {
            let mut execution = QueryProfileHandle::new(&profiler, identity).execution();
            drop(execution.scope());
            execution.finish();
        }

        let data = profiler.query_profiling_data(identity).unwrap();
        assert_eq!(data.count, 1);
    }

    #[test]
    fn finished_query_execution_cannot_open_another_scope() {
        let profiler = QueryProfiler::default();
        let identity = test_query_identity("NoScopeAfterFinish: (Age)");
        let mut execution = QueryProfileHandle::new(&profiler, identity).execution();
        drop(execution.scope());
        execution.finish();

        assert!(execution.scope().is_none());
        drop(execution);
        let data = profiler.query_profiling_data(identity).unwrap();
        assert_eq!(data.count, 1);
    }

    #[test]
    fn panicking_query_execution_scope_records_elapsed_work() {
        use std::panic::{catch_unwind, AssertUnwindSafe};

        let profiler = QueryProfiler::default();
        let identity = test_query_identity("PanickingQueryExecution: (Age)");
        let result = catch_unwind(AssertUnwindSafe(|| {
            let mut execution = QueryProfileHandle::new(&profiler, identity).execution();
            let _scope = execution.scope();
            panic!("end the measured work slice");
        }));

        assert!(result.is_err());
        let data = profiler.query_profiling_data(identity).unwrap();
        assert_eq!(data.count, 1);
    }
}
