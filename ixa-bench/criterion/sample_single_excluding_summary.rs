use std::collections::BTreeMap;

pub type Measurements = BTreeMap<(&'static str, usize), f64>;

#[derive(Debug, PartialEq)]
pub struct SummaryRow {
    pub size: usize,
    pub rejection: f64,
    pub iteration: f64,
    pub automatic: f64,
}

pub fn complete_rows(results: &Measurements, slice_sizes: &[usize]) -> Vec<SummaryRow> {
    slice_sizes
        .iter()
        .filter_map(|&size| {
            Some(SummaryRow {
                size,
                rejection: *results.get(&("rejection", size))?,
                iteration: *results.get(&("iteration", size))?,
                automatic: *results.get(&("auto", size))?,
            })
        })
        .collect()
}
