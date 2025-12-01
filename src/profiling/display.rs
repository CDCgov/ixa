#[cfg(feature = "profiling")]
use super::{profiling_data, ProfilingData, NAMED_COUNTS_HEADERS, NAMED_SPANS_HEADERS};
#[cfg(feature = "profiling")]
use humantime::format_duration;

/// Prints all collected profiling data.
#[cfg(feature = "profiling")]
pub fn print_profiling_data() {
    print_named_spans();
    print_named_counts();
    print_computed_statistics();
}

#[cfg(not(feature = "profiling"))]
pub fn print_profiling_data() {}

/// Prints a table of the named counts, if any.
#[cfg(feature = "profiling")]
pub fn print_named_counts() {
    let container = profiling_data();
    if container.counts.is_empty() {
        // nothing to report
        return;
    }
    let rows = container.get_named_counts_table();

    let mut formatted_rows = vec![
        // The header row
        NAMED_COUNTS_HEADERS
            .iter()
            .map(|s| (*s).to_string())
            .collect(),
    ];

    formatted_rows.extend(rows.into_iter().map(|(label, count, rate)| {
        vec![
            label,
            format_with_commas(count),
            format_with_commas_f64(rate),
        ]
    }));

    println!();
    print_formatted_table(&formatted_rows);
}

#[cfg(not(feature = "profiling"))]
pub fn print_named_counts() {}

/// Prints a table of the spans, if any.
#[cfg(feature = "profiling")]
pub fn print_named_spans() {
    let rows = profiling_data().get_named_spans_table();
    if rows.is_empty() {
        // nothing to report
        return;
    }

    let mut formatted_rows = vec![
        // Header row
        NAMED_SPANS_HEADERS
            .iter()
            .map(|s| (*s).to_string())
            .collect(),
    ];

    formatted_rows.extend(
        rows.into_iter()
            .map(|(label, count, duration, percent_runtime)| {
                vec![
                    label,
                    format_with_commas(count),
                    format_duration(duration).to_string(),
                    format!("{:.2}%", percent_runtime),
                ]
            }),
    );

    println!();
    print_formatted_table(&formatted_rows);
}

#[cfg(not(feature = "profiling"))]
pub fn print_named_spans() {}

/// Prints the forecast efficiency.
#[cfg(feature = "profiling")]
pub fn print_computed_statistics() {
    let mut container = profiling_data();

    // Compute first to avoid double borrow
    let stat_count = container.computed_statistics.len();
    if stat_count == 0 {
        return;
    }
    for idx in 0..stat_count {
        // Temporarily take the statistic, because we need immutable access to `container`.
        let mut statistic = container.computed_statistics[idx].take().unwrap();
        statistic.value = statistic.functions.compute(&container);
        // Return the statistic
        container.computed_statistics[idx] = Some(statistic);
    }

    println!();

    for statistic in &container.computed_statistics {
        let statistic = statistic.as_ref().unwrap();
        if statistic.value.is_none() {
            continue;
        }
        statistic.functions.print(statistic.value.unwrap());
    }
}
#[cfg(not(feature = "profiling"))]
pub fn print_computed_statistics() {}

/// Prints a table with aligned columns, using the first row as a header.
/// The first column is left-aligned; remaining columns are right-aligned.
/// Automatically adjusts column widths and inserts a separator line.
#[cfg(feature = "profiling")]
pub fn print_formatted_table(rows: &[Vec<String>]) {
    if rows.len() < 2 {
        return;
    }

    let num_cols = rows[0].len();
    let mut col_widths = vec![0; num_cols];

    // Compute max column widths
    for row in rows {
        for (i, cell) in row.iter().enumerate() {
            col_widths[i] = col_widths[i].max(cell.len());
        }
    }

    // Print header row
    let header = &rows[0];
    for (i, cell) in header.iter().enumerate() {
        if i == 0 {
            print!("{:<width$} ", cell, width = col_widths[i] + 1);
        } else {
            print!("{:>width$} ", cell, width = col_widths[i] + 1);
        }
    }
    println!();

    // Print separator
    let total_width: usize = col_widths.iter().map(|w| *w + 1).sum::<usize>() + 2;
    println!("{}", "-".repeat(total_width));

    // Print data rows
    for row in &rows[1..] {
        // First column left-aligned, rest right-aligned
        for (i, cell) in row.iter().enumerate() {
            if i == 0 {
                print!("{:<width$} ", cell, width = col_widths[i] + 1);
            } else {
                print!("{:>width$} ", cell, width = col_widths[i] + 1);
            }
        }
        println!();
    }
}

/// Formats an integer with thousands separator.
#[cfg(feature = "profiling")]
pub fn format_with_commas(value: usize) -> String {
    let s = value.to_string();
    let mut result = String::new();
    let bytes = s.as_bytes();
    let len = bytes.len();

    for (i, &b) in bytes.iter().enumerate() {
        result.push(b as char);
        let digits_left = len - i - 1;
        if digits_left > 0 && digits_left.is_multiple_of(3) {
            result.push(',');
        }
    }

    result
}

/// Formats a float with thousands separator.
#[cfg(feature = "profiling")]
pub fn format_with_commas_f64(value: f64) -> String {
    // Format to two decimal places
    let formatted = format!("{:.2}", value.abs()); // format positive part only
    let mut parts = formatted.splitn(2, '.');

    let int_part = parts.next().unwrap_or("");
    let frac_part = parts.next(); // optional

    // Format integer part with commas
    let mut result = String::new();
    let bytes = int_part.as_bytes();
    let len = bytes.len();

    for (i, &b) in bytes.iter().enumerate() {
        result.push(b as char);
        let digits_left = len - i - 1;
        if digits_left > 0 && digits_left % 3 == 0 {
            result.push(',');
        }
    }

    // Add decimal part
    if let Some(frac) = frac_part {
        result.push('.');
        result.push_str(frac);
    }

    // Reapply negative sign if needed
    if value.is_sign_negative() {
        result.insert(0, '-');
    }

    result
}

#[cfg(all(test, feature = "profiling"))]
mod tests {
    #![allow(clippy::unreadable_literal)]
    use crate::computed_statistics::{ACCEPTED_INFECTION_LABEL, FORECASTED_INFECTION_LABEL};
    use crate::profiling::display::{
        format_with_commas, format_with_commas_f64, print_named_counts, print_named_spans,
    };
    use crate::profiling::increment_named_count;
    use crate::profiling::*;
    use std::time::Duration;

    #[test]
    fn increments_named_count_correctly() {
        increment_named_count("test_event");
        increment_named_count("test_event");
        increment_named_count("another_event");

        let data = profiling_data();
        assert_eq!(data.get_named_count("test_event"), Some(2));
        assert_eq!(data.get_named_count("another_event"), Some(1));
    }

    #[test]
    fn print_named_counts_outputs_expected_format() {
        {
            // Inject a fixed start time 1 second ago
            let mut data = profiling_data();
            data.start_time = Some(Instant::now().checked_sub(Duration::from_secs(1)).unwrap());
            data.counts.insert("event1", 5);
        }
        print_named_counts(); // should print " event1  5  5.00 per second"
    }

    // region Tests for `format_with_commas()`
    #[test]
    fn formats_single_digit() {
        assert_eq!(format_with_commas(7), "7");
    }

    #[test]
    fn formats_two_digits() {
        assert_eq!(format_with_commas(42), "42");
    }

    #[test]
    fn formats_three_digits() {
        assert_eq!(format_with_commas(999), "999");
    }

    #[test]
    fn formats_four_digits() {
        assert_eq!(format_with_commas(1000), "1,000");
    }

    #[test]
    fn formats_five_digits() {
        assert_eq!(format_with_commas(27_171), "27,171");
    }

    #[test]
    fn formats_six_digits() {
        assert_eq!(format_with_commas(123_456), "123,456");
    }

    #[test]
    fn formats_seven_digits() {
        assert_eq!(format_with_commas(1_000_000), "1,000,000");
    }

    #[test]
    fn formats_zero() {
        assert_eq!(format_with_commas(0), "0");
    }

    #[test]
    fn formats_large_number() {
        assert_eq!(format_with_commas(9_876_543_210), "9,876,543,210");
    }

    // endregion Tests for `format_with_commas()`

    // region Tests for `format_with_commas_f64()`
    #[test]
    fn formats_small_integer() {
        assert_eq!(format_with_commas_f64(7.0), "7.00");
        assert_eq!(format_with_commas_f64(42.0), "42.00");
    }

    #[test]
    fn formats_small_decimal() {
        #![allow(clippy::approx_constant)]
        assert_eq!(format_with_commas_f64(3.14), "3.14");
        assert_eq!(format_with_commas_f64(0.99), "0.99");
    }

    #[test]
    fn formats_zero_f64() {
        assert_eq!(format_with_commas_f64(0.0), "0.00");
    }

    #[test]
    fn formats_exact_thousand() {
        assert_eq!(format_with_commas_f64(1000.0), "1,000.00");
    }

    #[test]
    fn formats_large_number_f64() {
        assert_eq!(format_with_commas_f64(1234567.89), "1,234,567.89");
        assert_eq!(format_with_commas_f64(123456789.0), "123,456,789.00");
    }

    #[test]
    fn formats_number_with_rounding_up() {
        assert_eq!(format_with_commas_f64(999.999), "1,000.00");
        assert_eq!(format_with_commas_f64(999999.999), "1,000,000.00");
    }

    #[test]
    fn formats_number_with_rounding_down() {
        assert_eq!(format_with_commas_f64(1234.444), "1,234.44");
    }

    #[test]
    fn formats_negative_number() {
        assert_eq!(format_with_commas_f64(-1234567.89), "-1,234,567.89");
    }

    #[test]
    fn formats_negative_rounding_edge() {
        assert_eq!(format_with_commas_f64(-999.995), "-1,000.00");
    }

    // endregion Tests for `format_with_commas_f64()`

    #[test]
    fn print_named_spans_outputs_expected_format() {
        {
            let mut container = profiling_data();

            // Set a fixed start time 10 seconds ago
            container.start_time =
                Some(Instant::now().checked_sub(Duration::from_secs(10)).unwrap());

            // Add sample spans data
            container
                .spans
                .insert("database_query", (Duration::from_millis(1500), 42));
            container
                .spans
                .insert("api_request", (Duration::from_millis(800), 120));
            container
                .spans
                .insert("data_processing", (Duration::from_secs(5), 15));
            container
                .spans
                .insert("file_io", (Duration::from_millis(350), 78));
            container
                .spans
                .insert("rendering", (Duration::from_secs(2), 30));
        }
        print_named_spans();
    }

    #[test]
    fn test_print_computed_statistics_integration() {
        use crate::profiling::{add_computed_statistic, increment_named_count};
        {
            let mut data = profiling_data();
            data.counts.clear();
            data.computed_statistics.clear();
        }

        increment_named_count("metric");
        increment_named_count("metric");

        add_computed_statistic::<usize>(
            "metric_count",
            "Total metrics",
            Box::new(|data| data.get_named_count("metric")),
            Box::new(|value| {
                println!("Metric count: {}", value);
            }),
        );

        print_computed_statistics();
    }
}
