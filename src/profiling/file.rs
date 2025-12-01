use super::computed_statistic::{ComputedStatistic, ComputedValue};
#[cfg(feature = "profiling")]
use super::profiling_data;
use ixa::execution_stats::ExecutionStatistics;
use ixa::HashMap;
#[cfg(feature = "profiling")]
use serde::{Serialize, Serializer};
use std::path::Path;
#[cfg(feature = "profiling")]
use std::{
    fs::File,
    io::Write,
    time::{Duration, SystemTime},
};

/// A wrapper around Duration the serialization format of which we have control over.
#[cfg(feature = "profiling")]
#[derive(Debug, Copy, Clone)]
struct SerializableDuration(pub Duration);

#[cfg(feature = "profiling")]
impl Serialize for SerializableDuration {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_f64(self.0.as_secs_f64())
    }
}

/// A version of `ExecutionStatistics` the serialization format of which we have control over.
#[cfg(feature = "profiling")]
#[derive(Serialize)]
struct SerializableExecutionStatistics {
    max_memory_usage: u64,
    cpu_time: SerializableDuration,
    wall_time: SerializableDuration,

    // Per person stats
    population: usize,
    cpu_time_per_person: SerializableDuration,
    wall_time_per_person: SerializableDuration,
    memory_per_person: u64,
}

#[cfg(feature = "profiling")]
impl From<ExecutionStatistics> for SerializableExecutionStatistics {
    fn from(value: ExecutionStatistics) -> Self {
        SerializableExecutionStatistics {
            max_memory_usage: value.max_memory_usage,
            cpu_time: SerializableDuration(value.cpu_time),
            wall_time: SerializableDuration(value.wall_time),
            population: value.population,
            cpu_time_per_person: SerializableDuration(value.cpu_time_per_person),
            wall_time_per_person: SerializableDuration(value.wall_time_per_person),
            memory_per_person: value.memory_per_person,
        }
    }
}

#[cfg(feature = "profiling")]
#[derive(Serialize)]
struct SpanRecord {
    label: String,
    count: usize,
    duration: SerializableDuration,
    percent_runtime: f64,
}

#[cfg(feature = "profiling")]
#[derive(Serialize)]
struct CountRecord {
    label: String,
    count: usize,
    rate_per_second: f64,
}

#[cfg(feature = "profiling")]
#[derive(Serialize)]
struct ProfilingDataRecord {
    date_time: SystemTime,
    execution_statistics: SerializableExecutionStatistics,
    named_counts: Vec<CountRecord>,
    named_spans: Vec<SpanRecord>,
    computed_statistics: HashMap<&'static str, ComputedStatisticRecord>,
}

#[cfg(feature = "profiling")]
#[derive(Serialize)]
struct ComputedStatisticRecord {
    description: &'static str,
    value: ComputedValue,
}

#[cfg(feature = "profiling")]
pub fn write_profiling_data_to_file<P: AsRef<Path>>(
    file_path: P,
    execution_statistics: ExecutionStatistics,
) -> std::io::Result<()> {
    let mut container = profiling_data();
    let named_spans_data = container.get_named_spans_table();
    let named_spans_data = named_spans_data
        .into_iter()
        .map(|(label, count, duration, percent_runtime)| SpanRecord {
            label,
            count,
            duration: SerializableDuration(duration),
            percent_runtime,
        })
        .collect();
    let named_counts_data = container.get_named_counts_table();
    let named_counts_data = named_counts_data
        .into_iter()
        .map(|(label, count, rate_per_second)| CountRecord {
            label,
            count,
            rate_per_second,
        })
        .collect();

    // Compute first to avoid double borrow
    let stat_count = container.computed_statistics.len();
    for idx in 0..stat_count {
        // Temporarily take the statistic, because we need immutable access to `container`.
        let mut statistic = container.computed_statistics[idx].take().unwrap();
        statistic.value = statistic.functions.compute(&container);
        // Return the statistic
        container.computed_statistics[idx] = Some(statistic);
    }

    let computed_statistics = container.computed_statistics.iter().filter_map(|stat| {
        let stat = stat.as_ref().unwrap();
        if stat.value.is_none() {
            None
        } else {
            Some((
                stat.label,
                ComputedStatisticRecord {
                    description: stat.description,
                    value: stat.value.unwrap(),
                },
            ))
        }
    });
    let computed_statistics = computed_statistics.collect::<HashMap<_, _>>();

    let profiling_data = ProfilingDataRecord {
        date_time: SystemTime::now(),
        execution_statistics: execution_statistics.into(),
        named_counts: named_counts_data,
        named_spans: named_spans_data,
        computed_statistics,
    };

    let json =
        serde_json::to_string_pretty(&profiling_data).expect("ProfilingData serialization failed");

    let mut file = File::create(file_path)?;
    file.write_all(json.as_bytes())?;
    Ok(())
}

#[cfg(not(feature = "profiling"))]
pub fn write_profiling_data_to_file<P: AsRef<Path>>(
    _file_path: P,
    _execution_statistics: ExecutionStatistics,
) -> std::io::Result<()> {
    Ok(())
}

#[cfg(all(test, feature = "profiling"))]
mod tests {
    use super::*;
    use crate::profiling::{add_computed_statistic, get_profiling_data, increment_named_count, open_span};
    use std::fs;
    use std::time::Duration;
    use tempfile::TempDir;

    #[test]
    fn test_write_profiling_data_to_file() {
        {
            let mut data = get_profiling_data();
            data.counts.clear();
            data.spans.clear();
            data.computed_statistics.clear();
        }

        increment_named_count("test_event");
        increment_named_count("test_event");
        {
            let _span = open_span("test_span");
            std::thread::sleep(Duration::from_millis(10));
        }

        add_computed_statistic::<usize>(
            "event_count",
            "Total test events",
            Box::new(|data| data.get_named_count("test_event")),
            Box::new(|value| println!("Events: {}", value)),
        );

        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("profiling_test.json");

        let exec_stats = ExecutionStatistics {
            max_memory_usage: 1024 * 1024,
            cpu_time: Duration::from_secs(1),
            wall_time: Duration::from_secs(2),
            population: 1000,
            cpu_time_per_person: Duration::from_micros(1000),
            wall_time_per_person: Duration::from_micros(2000),
            memory_per_person: 1024,
        };

        write_profiling_data_to_file(&file_path, exec_stats).expect("Failed to write file");

        assert!(file_path.exists());

        let content = fs::read_to_string(&file_path).expect("Failed to read file");
        let json: serde_json::Value = serde_json::from_str(&content).expect("Invalid JSON");

        assert!(json["date_time"].is_string());
        assert!(json["execution_statistics"].is_object());
        assert!(json["named_counts"].is_array());
        assert!(json["named_spans"].is_array());
        assert!(json["computed_statistics"].is_object());

        assert_eq!(json["execution_statistics"]["population"], 1000);
        assert_eq!(json["execution_statistics"]["max_memory_usage"], 1024 * 1024);

        let counts = json["named_counts"].as_array().unwrap();
        assert!(counts.len() >= 1);
        let test_event = counts
            .iter()
            .find(|c| c["label"] == "test_event")
            .expect("test_event not found");
        assert_eq!(test_event["count"], 2);

        let computed = &json["computed_statistics"];
        assert!(computed["event_count"].is_object());
        assert_eq!(computed["event_count"]["description"], "Total test events");
        assert_eq!(computed["event_count"]["value"], 2);
    }

    #[test]
    fn test_json_serialization_format() {
        {
            let mut data = get_profiling_data();
            data.counts.clear();
            data.spans.clear();
            data.computed_statistics.clear();
        }

        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("format_test.json");

        let exec_stats = ExecutionStatistics {
            max_memory_usage: 2048,
            cpu_time: Duration::from_secs_f64(1.5),
            wall_time: Duration::from_secs_f64(2.5),
            population: 500,
            cpu_time_per_person: Duration::from_micros(3000),
            wall_time_per_person: Duration::from_micros(5000),
            memory_per_person: 4,
        };

        write_profiling_data_to_file(&file_path, exec_stats).expect("Failed to write file");

        let content = fs::read_to_string(&file_path).expect("Failed to read file");
        let json: serde_json::Value = serde_json::from_str(&content).expect("Invalid JSON");

        let cpu_time = json["execution_statistics"]["cpu_time"].as_f64().unwrap();
        assert!((cpu_time - 1.5).abs() < 0.01);

        let wall_time = json["execution_statistics"]["wall_time"].as_f64().unwrap();
        assert!((wall_time - 2.5).abs() < 0.01);
    }
}
