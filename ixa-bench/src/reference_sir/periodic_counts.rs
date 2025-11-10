use ixa::prelude::*;
use ixa::PersonId;
use rand_distr::Exp;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tempfile::TempDir;

use super::Parameters;

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
    fn infected_people(&self) -> usize;
    fn random_person(&mut self) -> Option<PersonId>;
    fn random_infected_person(&mut self) -> Option<PersonId>;
    fn infect_person(&mut self, p: PersonId);
    fn recover_person(&mut self, p: PersonId);
    fn next_event(&mut self);
    fn setup(&mut self, temp_dir: Option<&TempDir>);
}

impl InfectionLoop for Context {
    fn get_params(&self) -> &Parameters {
        self.get_global_property_value(PeriodicParams).unwrap()
    }
    fn get_options(&self) -> &ModelOptions {
        self.get_global_property_value(PeriodicOptions).unwrap()
    }
    fn infected_people(&self) -> usize {
        #[allow(deprecated)]
        self.query_people_count((InfectionStatus, InfectionStatusValue::Infectious))
    }
    fn random_person(&mut self) -> Option<PersonId> {
        self.sample_person(PeriodicNextPersonRng, ())
    }
    fn random_infected_person(&mut self) -> Option<PersonId> {
        self.sample_person(
            PeriodicNextPersonRng,
            (InfectionStatus, InfectionStatusValue::Infectious),
        )
    }
    fn infect_person(&mut self, p: PersonId) {
        if self.get_person_property(p, InfectionStatus) != InfectionStatusValue::Susceptible {
            return;
        }
        self.set_person_property(p, InfectionStatus, InfectionStatusValue::Infectious);
    }

    fn recover_person(&mut self, p: PersonId) {
        self.set_person_property(p, InfectionStatus, InfectionStatusValue::Recovered);
    }
    fn next_event(&mut self) {
        let params = self.get_params();
        let infection_rate = params.r0 / params.infectious_period;
        let n = self.infected_people() as f64;

        if n == 0.0 {
            return;
        }

        let infection_event_rate = infection_rate * n;
        let recovery_event_rate = n / params.infectious_period;

        let infection_event_time =
            self.sample_distr(PeriodicNextEventRng, Exp::new(infection_event_rate).unwrap());
        let recovery_event_time =
            self.sample_distr(PeriodicNextEventRng, Exp::new(recovery_event_rate).unwrap());

        let p = self.random_person().unwrap();
        if infection_event_time < recovery_event_time {
            if self.get_person_property(p, InfectionStatus) == InfectionStatusValue::Susceptible {
                self.add_plan(
                    self.get_current_time() + infection_event_time,
                    move |context| {
                        context.infect_person(p);
                        if context.infected_people() > 0 {
                            context.next_event();
                        }
                    },
                );
                return;
            }
        } else {
            self.add_plan(self.get_current_time() + recovery_event_time, |context| {
                if let Some(p) = context.random_infected_person() {
                    context.recover_person(p);
                }
                if context.infected_people() > 0 {
                    context.next_event();
                }
            });
            return;
        }

        self.next_event();
    }
    fn setup(&mut self, temp_dir: Option<&TempDir>) {
        let &Parameters {
            population,
            initial_infections,
            seed,
            max_time,
            ..
        } = self.get_params();
        let &ModelOptions {
            periodic_reporting,
        } = self.get_options();

        self.init_random(seed);
        self.index_property(InfectionStatus);

        // Set up population with ages
        for i in 0..population {
            // Assign ages roughly evenly across 0-100
            let age = (i % 101) as u8;
            self.add_person(((Age, age),)).unwrap();
        }

        // Seed infections
        for p in self.sample_people(
            PeriodicNextPersonRng,
            (InfectionStatus, InfectionStatusValue::Susceptible),
            initial_infections,
        ) {
            self.infect_person(p);
        }

        // Set up periodic reporting if enabled
        if periodic_reporting {
            if let Some(dir) = temp_dir {
                let output_dir = PathBuf::from(dir.path());
                
                self.report_options()
                    .directory(output_dir)
                    .overwrite(true);

                // Add periodic report for infection status
                self.add_periodic_report("daily_infections", 1.0, (InfectionStatus,))
                    .expect("Failed to add periodic report for infections");

                // Add periodic report for infections by age group
                self.add_periodic_report("infections_by_age", 1.0, (InfectionStatus, Age))
                    .expect("Failed to add periodic report for infections by age");
            }
        }

        self.add_plan(max_time, |context| {
            context.shutdown();
        });

        assert_eq!(
            self.infected_people(),
            initial_infections,
            "should have infected people at start"
        );
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
        
        ctx.set_global_property_value(PeriodicParams, params).unwrap();
        ctx.set_global_property_value(PeriodicOptions, options).unwrap();
        
        Self { ctx, temp_dir }
    }
    pub fn run(&mut self) {
        self.ctx.setup(self.temp_dir.as_ref());
        self.ctx.next_event();
        self.ctx.execute();
    }
}

#[cfg(test)]
mod test {
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
}
