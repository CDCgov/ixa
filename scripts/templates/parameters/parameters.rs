use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Parameters {
    /// New infections per unit time per infectious individual.
    pub infection_rate: f64,
    /// Mean time an individual remains infectious.
    pub infectious_period: f64,
    pub population: usize,
    pub initial_infections: usize,
    pub seed: u64,
    pub max_time: f64,
}

impl Default for Parameters {
    fn default() -> Self {
        Self {
            infection_rate: 0.5,
            infectious_period: 3.0,
            population: 10_000,
            initial_infections: 5,
            seed: 0,
            max_time: 100.0,
        }
    }
}

impl Parameters {
    pub fn validate(&self) -> Result<(), String> {
        if !self.infection_rate.is_finite() || self.infection_rate < 0.0 {
            return Err(format!(
                "infection_rate must be a finite non-negative number, got {}",
                self.infection_rate
            ));
        }
        if !self.infectious_period.is_finite() || self.infectious_period <= 0.0 {
            return Err(format!(
                "infectious_period must be a finite positive number, got {}",
                self.infectious_period
            ));
        }
        if !self.max_time.is_finite() || self.max_time < 0.0 {
            return Err(format!(
                "max_time must be a finite non-negative number, got {}",
                self.max_time
            ));
        }
        if self.initial_infections > self.population {
            return Err(format!(
                "initial_infections ({}) must not exceed population ({})",
                self.initial_infections, self.population
            ));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_valid() {
        Parameters::default().validate().unwrap();
    }

    #[test]
    fn rejects_negative_infection_rate() {
        let parameters = Parameters {
            infection_rate: -1.0,
            ..Parameters::default()
        };
        assert!(parameters.validate().is_err());
    }

    #[test]
    fn rejects_non_finite_infection_rate() {
        let parameters = Parameters {
            infection_rate: f64::NAN,
            ..Parameters::default()
        };
        assert!(parameters.validate().is_err());
    }

    #[test]
    fn accepts_zero_infection_rate() {
        let parameters = Parameters {
            infection_rate: 0.0,
            ..Parameters::default()
        };
        parameters.validate().unwrap();
    }

    #[test]
    fn rejects_zero_infectious_period() {
        let parameters = Parameters {
            infectious_period: 0.0,
            ..Parameters::default()
        };
        assert!(parameters.validate().is_err());
    }

    #[test]
    fn rejects_non_finite_infectious_period() {
        let parameters = Parameters {
            infectious_period: f64::INFINITY,
            ..Parameters::default()
        };
        assert!(parameters.validate().is_err());
    }

    #[test]
    fn rejects_negative_max_time() {
        let parameters = Parameters {
            max_time: -1.0,
            ..Parameters::default()
        };
        assert!(parameters.validate().is_err());
    }

    #[test]
    fn rejects_non_finite_max_time() {
        let parameters = Parameters {
            max_time: f64::NAN,
            ..Parameters::default()
        };
        assert!(parameters.validate().is_err());
    }

    #[test]
    fn rejects_initial_infections_exceeding_population() {
        let parameters = Parameters {
            population: 10,
            initial_infections: 11,
            ..Parameters::default()
        };
        assert!(parameters.validate().is_err());
    }
}
