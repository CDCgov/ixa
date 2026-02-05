use std::path::PathBuf;

use ixa::prelude::*;
use ixa::PersonId;
use serde::{Deserialize, Serialize};
use tempfile::TempDir;

use super::Parameters;
use crate::generate_population::generate_population_with_seed;

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct ModelOptions {
    pub periodic_reporting: bool,
}

impl Default for ModelOptions {
    fn default() -> Self {
        ModelOptions {
            periodic_reporting: true,
        }
    }
}

define_global_property!(PeriodicParams, Parameters);
define_global_property!(PeriodicOptions, ModelOptions);

#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize)]
pub enum InfectionStatusValue {
    Susceptible,
    Infectious,
    Recovered,
}

define_person_property_with_default!(
    InfectionStatus,
    InfectionStatusValue,
    InfectionStatusValue::Susceptible
);

// Age property for age-grouped counting
define_person_property!(Age, u8);

define_rng!(PeriodicNextPersonRng);
define_rng!(PeriodicNextEventRng);

trait InfectionLoop {
    fn get_params(&self) -> &Parameters;
    fn get_options(&self) -> &ModelOptions;
    fn infect_person(&mut self, p: PersonId);
    fn setup(&mut self, temp_dir: Option<&TempDir>);
}

impl InfectionLoop for Context {
    fn get_params(&self) -> &Parameters {
        self.get_global_property_value(PeriodicParams).unwrap()
    }

    fn get_options(&self) -> &ModelOptions {
        self.get_global_property_value(PeriodicOptions).unwrap()
    }

    fn infect_person(&mut self, p: PersonId) {
        if self.get_person_property(p, InfectionStatus) != InfectionStatusValue::Susceptible {
            return;
        }
        self.set_person_property(p, InfectionStatus, InfectionStatusValue::Infectious);
    }

    fn setup(&mut self, temp_dir: Option<&TempDir>) {
        let &Parameters {
            population,
            initial_infections,
            seed,
            max_time,
            ..
        } = self.get_params();
        let &ModelOptions { periodic_reporting } = self.get_options();
        println!("Setting up model: population={}, initial_infections={}, max_time={}, periodic_reporting={}",
            population, initial_infections, max_time, periodic_reporting);
        self.init_random(seed);

        // Set up population with ages using the population generator
        // Use small, sensible defaults for schools/workplaces percent of population
        const SCHOOLS_PERCENT: f64 = 0.2;
        const WORKPLACES_PERCENT: f64 = 10.0;
        for person in generate_population_with_seed(
            population,
            SCHOOLS_PERCENT,
            WORKPLACES_PERCENT,
            Some(seed),
        ) {
            // We currently only need the Age attribute for this benchmark
            let person = self.add_person(((Age, person.age),)).unwrap();
            self.add_plan(0.0, move |context| {
                context.infect_person(person);
            });
        }

        // Set up periodic reporting if enabled
        if periodic_reporting {
            if let Some(dir) = temp_dir {
                let output_dir = PathBuf::from(dir.path());

                self.report_options().directory(output_dir).overwrite(true);

                // Add periodic report for infections by age group
                self.add_periodic_report("infections_by_age", 1.0, (InfectionStatus, Age))
                    .expect("Failed to add periodic report for infections by age");
            }
        }

        self.add_plan(max_time, |context| {
            context.shutdown();
        });
    }
}

pub struct Model {
    ctx: Context,
    temp_dir: Option<TempDir>,
}

impl Model {
    pub fn new(params: Parameters, options: ModelOptions) -> Self {
        let mut ctx = Context::new();

        // Create temp directory for reports only if periodic reporting is enabled
        let temp_dir = if options.periodic_reporting {
            Some(TempDir::new().expect("Failed to create temp directory"))
        } else {
            None
        };

        ctx.set_global_property_value(PeriodicParams, params)
            .unwrap();
        ctx.set_global_property_value(PeriodicOptions, options)
            .unwrap();

        Self { ctx, temp_dir }
    }
    pub fn run(&mut self) {
        self.ctx.setup(self.temp_dir.as_ref());
        self.ctx.execute();
    }
}

#[cfg(test)]
mod test {
    use std::path::PathBuf;

    use super::super::ParametersBuilder;
    use super::*;

    #[test]
    fn run_model_with_periodic_reports() {
        let mut model = Model::new(
            ParametersBuilder::default()
                .population(1000)
                .initial_infections(10)
                .max_time(5.0)
                .build()
                .unwrap(),
            ModelOptions {
                periodic_reporting: true,
            },
        );
        model.run();
    }

    #[test]
    fn run_model_without_periodic_reports() {
        let mut model = Model::new(
            ParametersBuilder::default()
                .population(1000)
                .initial_infections(10)
                .max_time(5.0)
                .build()
                .unwrap(),
            ModelOptions {
                periodic_reporting: false,
            },
        );
        model.run();
    }

    #[test]
    fn verify_csv_output() {
        let temp_dir = TempDir::new().expect("Failed to create temp directory");
        let output_path = PathBuf::from(temp_dir.path());

        // We need the context to go out of scope so the CSV writers are flushed
        {
            let mut ctx = Context::new();
            ctx.set_global_property_value(
                PeriodicParams,
                ParametersBuilder::default()
                    .population(100)
                    .initial_infections(5)
                    .max_time(3.0)
                    .build()
                    .unwrap(),
            )
            .unwrap();
            ctx.set_global_property_value(
                PeriodicOptions,
                ModelOptions {
                    periodic_reporting: true,
                },
            )
            .unwrap();

            ctx.setup(Some(&temp_dir));
            ctx.execute();
        }

        // Now the context is dropped and files should be flushed
        let by_age_file = output_path.join("infections_by_age.csv");

        assert!(
            by_age_file.exists(),
            "infections_by_age.csv should be created"
        );

        // Verify infections_by_age.csv has expected structure
        let contents = std::fs::read_to_string(&by_age_file).unwrap();
        let lines: Vec<&str> = contents.lines().collect();

        // Check header exists
        assert!(!lines.is_empty(), "CSV should have at least a header");

        // Check header
        let header = lines[0];
        assert!(header.contains("t"), "Header should contain 't'");
        assert!(
            header.contains("InfectionStatus"),
            "Header should contain 'InfectionStatus'"
        );
        assert!(header.contains("Age"), "Header should contain 'Age'");
        assert!(header.contains("count"), "Header should contain 'count'");

        // Should have data rows
        assert!(
            lines.len() > 1,
            "CSV should have data rows in addition to header"
        );
    }
}
