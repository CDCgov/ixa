#[cfg(feature = "profiling")]
use super::profiling_data;
use super::ProfilingData;
use serde::Serialize;
use std::fmt::Display;

pub type CustomStatisticComputer<T> = Box<dyn (Fn(&ProfilingData) -> Option<T>) + Send + Sync>;
pub type CustomStatisticPrinter<T> = Box<dyn Fn(T) + Send + Sync>;

pub(super) enum ComputedStatisticFunctions {
    USize {
        computer: CustomStatisticComputer<usize>,
        printer: CustomStatisticPrinter<usize>,
    },
    Int {
        computer: CustomStatisticComputer<i64>,
        printer: CustomStatisticPrinter<i64>,
    },
    Float {
        computer: CustomStatisticComputer<f64>,
        printer: CustomStatisticPrinter<f64>,
    },
}

impl ComputedStatisticFunctions {
    /// A type erased way to compute a statistic.
    pub(super) fn compute(&self, container: &ProfilingData) -> Option<ComputedValue> {
        match self {
            ComputedStatisticFunctions::USize { computer, .. } => {
                computer(container).map(ComputedValue::USize)
            }
            ComputedStatisticFunctions::Int { computer, .. } => {
                computer(container).map(ComputedValue::Int)
            }
            ComputedStatisticFunctions::Float { computer, .. } => {
                computer(container).map(ComputedValue::Float)
            }
        }
    }

    /// A type erased way to print a statistic.
    pub(super) fn print(&self, value: ComputedValue) {
        match value {
            ComputedValue::USize(value) => {
                let ComputedStatisticFunctions::USize { printer, .. } = self else {
                    unreachable!()
                };
                (printer)(value);
            }
            ComputedValue::Int(value) => {
                let ComputedStatisticFunctions::Int { printer, .. } = self else {
                    unreachable!()
                };
                (printer)(value);
            }
            ComputedValue::Float(value) => {
                let ComputedStatisticFunctions::Float { printer, .. } = self else {
                    unreachable!()
                };
                (printer)(value);
            }
        }
    }
}

pub(super) struct ComputedStatistic {
    /// The label used for the statistic in the JSON report.
    pub label: &'static str,
    /// Description of the statistic. Used in the JSON report.
    pub description: &'static str,
    /// The computed value of the statistic.
    pub value: Option<ComputedValue>,
    /// The two functions used to compute the statistic and to print it to the console.
    pub functions: ComputedStatisticFunctions,
}

// This trick makes it so client code can _use_ `ComputableType` but not _implement_ it.
mod sealed {
    pub(super) trait SealedComputableType {}
}
#[allow(private_bounds)]
pub trait ComputableType: sealed::SealedComputableType
where
    Self: Sized,
{
    // This method is only callable from within this crate.
    #[allow(private_interfaces)]
    fn new_functions(
        computer: CustomStatisticComputer<Self>,
        printer: CustomStatisticPrinter<Self>,
    ) -> ComputedStatisticFunctions;
}
impl sealed::SealedComputableType for usize {}
impl ComputableType for usize {
    #[allow(private_interfaces)]
    fn new_functions(
        computer: CustomStatisticComputer<Self>,
        printer: CustomStatisticPrinter<Self>,
    ) -> ComputedStatisticFunctions {
        ComputedStatisticFunctions::USize { computer, printer }
    }
}
impl sealed::SealedComputableType for i64 {}
impl ComputableType for i64 {
    #[allow(private_interfaces)]
    fn new_functions(
        computer: CustomStatisticComputer<Self>,
        printer: CustomStatisticPrinter<Self>,
    ) -> ComputedStatisticFunctions {
        ComputedStatisticFunctions::Int { computer, printer }
    }
}
impl sealed::SealedComputableType for f64 {}
impl ComputableType for f64 {
    #[allow(private_interfaces)]
    fn new_functions(
        computer: CustomStatisticComputer<Self>,
        printer: CustomStatisticPrinter<Self>,
    ) -> ComputedStatisticFunctions {
        ComputedStatisticFunctions::Float { computer, printer }
    }
}

/// The computed value of a statistic. The "computer" returns a value of this type.
#[derive(Copy, Clone, PartialEq, Serialize, Debug)]
pub(super) enum ComputedValue {
    USize(usize),
    Int(i64),
    Float(f64),
}

impl Display for ComputedValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ComputedValue::USize(value) => {
                write!(f, "{}", value)
            }

            ComputedValue::Int(value) => {
                write!(f, "{}", value)
            }

            ComputedValue::Float(value) => {
                write!(f, "{}", value)
            }
        }
    }
}

#[cfg(feature = "profiling")]
pub fn add_computed_statistic<T: ComputableType>(
    label: &'static str,
    description: &'static str,
    computer: CustomStatisticComputer<T>,
    printer: CustomStatisticPrinter<T>,
) {
    let mut container = profiling_data();
    container.add_computed_statistic(label, description, computer, printer);
}
#[cfg(not(feature = "profiling"))]
pub fn add_computed_statistic<T: ComputableType>(
    _label: &'static str,
    _description: &'static str,
    _computer: CustomStatisticComputer<T>,
    _printer: CustomStatisticPrinter<T>,
) {
}

#[cfg(all(test, feature = "profiling"))]
mod tests {
    use super::*;
    use crate::profiling::{get_profiling_data, increment_named_count};
    use std::sync::atomic::{AtomicBool, Ordering};

    #[test]
    fn test_computed_statistic_usize() {
        {
            let mut data = get_profiling_data();
            data.counts.clear();
            data.computed_statistics.clear();
        }

        increment_named_count("events");
        increment_named_count("events");
        increment_named_count("events");

        add_computed_statistic::<usize>(
            "total_events",
            "Total number of events",
            Box::new(|data| data.get_named_count("events")),
            Box::new(|value| println!("Total events: {}", value)),
        );

        let data = get_profiling_data();
        assert_eq!(data.computed_statistics.len(), 1);

        let stat = data.computed_statistics[0].as_ref().unwrap();
        let computed = stat.functions.compute(&data);
        assert_eq!(computed, Some(ComputedValue::USize(3)));
    }

    #[test]
    fn test_computed_statistic_i64() {
        {
            let mut data = get_profiling_data();
            data.counts.clear();
            data.computed_statistics.clear();
        }

        increment_named_count("positive");
        increment_named_count("positive");
        increment_named_count("negative");

        add_computed_statistic::<i64>(
            "difference",
            "Difference between positive and negative",
            Box::new(|data| {
                let pos = data.get_named_count("positive").unwrap_or(0) as i64;
                let neg = data.get_named_count("negative").unwrap_or(0) as i64;
                Some(pos - neg)
            }),
            Box::new(|value| println!("Difference: {}", value)),
        );

        let data = get_profiling_data();
        let stat = data.computed_statistics[0].as_ref().unwrap();
        let computed = stat.functions.compute(&data);
        assert_eq!(computed, Some(ComputedValue::Int(1)));
    }

    #[test]
    fn test_computed_statistic_f64() {
        {
            let mut data = get_profiling_data();
            data.counts.clear();
            data.computed_statistics.clear();
        }

        increment_named_count("successes");
        increment_named_count("successes");
        increment_named_count("successes");
        increment_named_count("total");
        increment_named_count("total");
        increment_named_count("total");
        increment_named_count("total");

        add_computed_statistic::<f64>(
            "success_rate",
            "Success rate as percentage",
            Box::new(|data| {
                let successes = data.get_named_count("successes")? as f64;
                let total = data.get_named_count("total")? as f64;
                Some(successes / total * 100.0)
            }),
            Box::new(|value| println!("Success rate: {:.2}%", value)),
        );

        let data = get_profiling_data();
        let stat = data.computed_statistics[0].as_ref().unwrap();
        let computed = stat.functions.compute(&data);
        if let Some(ComputedValue::Float(value)) = computed {
            assert!((value - 75.0).abs() < 0.01);
        } else {
            panic!("Expected Float value");
        }
    }

    #[test]
    fn test_computed_statistic_returns_none() {
        {
            let mut data = get_profiling_data();
            data.counts.clear();
            data.computed_statistics.clear();
        }

        add_computed_statistic::<usize>(
            "missing_data",
            "Statistic with missing data",
            Box::new(|data| data.get_named_count("nonexistent")),
            Box::new(|value| println!("Value: {}", value)),
        );

        let data = get_profiling_data();
        let stat = data.computed_statistics[0].as_ref().unwrap();
        let computed = stat.functions.compute(&data);
        assert_eq!(computed, None);
    }

    #[test]
    fn test_computed_value_display() {
        let usize_val = ComputedValue::USize(42);
        assert_eq!(format!("{}", usize_val), "42");

        let int_val = ComputedValue::Int(-100);
        assert_eq!(format!("{}", int_val), "-100");

        let float_val = ComputedValue::Float(3.14159);
        assert_eq!(format!("{}", float_val), "3.14159");
    }

    #[test]
    fn test_computed_statistic_print_functions() {
        static PRINTED: AtomicBool = AtomicBool::new(false);

        {
            let mut data = get_profiling_data();
            data.counts.clear();
            data.computed_statistics.clear();
        }

        increment_named_count("test");

        add_computed_statistic::<usize>(
            "test_stat",
            "Test statistic",
            Box::new(|data| data.get_named_count("test")),
            Box::new(|_value| {
                PRINTED.store(true, Ordering::SeqCst);
            }),
        );

        let mut data = get_profiling_data();
        let stat = data.computed_statistics[0].take().unwrap();
        let value = stat.functions.compute(&data).unwrap();
        stat.functions.print(value);

        assert!(PRINTED.load(Ordering::SeqCst));
    }
}
