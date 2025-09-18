use std::collections::BTreeMap;

pub struct ModelStats {
    cum_incidence: usize,
    cum_incidence_timeseries: BTreeMap<usize, usize>,
    current_infected: usize,
    population_size: usize,
    extinction_threshold: f64,
}

#[derive(Debug, PartialEq, Eq, Clone, Copy, Hash)]
pub struct Row {
    pub t: usize,
    pub count: usize,
}

impl ModelStats {
    pub fn new(
        initial_infections: usize,
        population_size: usize,
        extinction_threshold: f64,
    ) -> Self {
        Self {
            cum_incidence: 0,
            cum_incidence_timeseries: BTreeMap::from([(0, 0)]),
            current_infected: initial_infections,
            population_size,
            extinction_threshold,
        }
    }
    pub fn set_current_infected(&mut self, value: usize) {
        self.current_infected = value;
    }
    pub fn record_recovery(&mut self) {
        self.current_infected -= 1;
    }
    pub fn record_infection(&mut self, current_t: f64) {
        self.cum_incidence += 1;
        self.current_infected += 1;

        let t_floor = current_t.floor() as usize;
        self.cum_incidence_timeseries
            .entry(t_floor)
            .and_modify(|e| *e += 1)
            .or_insert(self.cum_incidence);
    }
    pub fn get_cum_incidence(&self) -> usize {
        self.cum_incidence
    }
    pub fn get_current_infected(&self) -> usize {
        self.current_infected
    }
    pub fn get_timeseries(&self) -> Vec<Row> {
        self.cum_incidence_timeseries
            .iter()
            .map(|(&k, &v)| Row { t: k, count: v })
            .collect()
    }
    pub fn check_extinction(&self) {
        assert!(
            self.get_cum_incidence() as f64
                > self.population_size as f64 * self.extinction_threshold,
            "Epidemic did not take off, only {} infections in total. Maybe you need another seed?",
            self.get_cum_incidence()
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_model_stats() {
        let mut stats = ModelStats::new(5, 1000, 0.2);
        assert_eq!(stats.get_cum_incidence(), 0);
        assert_eq!(stats.get_current_infected(), 5);

        stats.record_infection(1.2);
        stats.record_infection(2.8);

        assert_eq!(stats.get_cum_incidence(), 2);
        assert_eq!(stats.get_current_infected(), 7);
        assert_eq!(
            stats.get_timeseries(),
            vec![
                Row { t: 0, count: 0 },
                Row { t: 1, count: 1 },
                Row { t: 2, count: 2 },
            ]
        );

        stats.record_recovery();
        assert_eq!(stats.get_cum_incidence(), 2);
        assert_eq!(stats.get_current_infected(), 6);
    }
}
