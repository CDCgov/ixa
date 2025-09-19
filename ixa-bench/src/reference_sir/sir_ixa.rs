use super::{ModelStats, Parameters};
use indexmap::IndexSet;
use ixa::{prelude::*, PersonId};
use rand_distr::Exp;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct ModelOptions {
    pub queries_enabled: bool,
}
impl Default for ModelOptions {
    fn default() -> Self {
        ModelOptions {
            queries_enabled: true,
        }
    }
}

define_global_property!(Params, Parameters);
define_global_property!(Options, ModelOptions);

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

define_rng!(NextPersonRng);
define_rng!(NextEventRng);

define_data_plugin!(ModelStatsPlugin, ModelStats, |context| {
    let params = context.get_global_property_value(Params).unwrap();
    ModelStats::new(params.initial_infections, params.population, 0.2)
});
define_data_plugin!(
    NonQueryInfectionTracker,
    IndexSet<PersonId>,
    IndexSet::new()
);

trait InfectionLoop {
    fn get_params(&self) -> &Parameters;
    fn get_options(&self) -> &ModelOptions;
    fn get_stats(&self) -> &ModelStats;
    fn infected_people(&self) -> usize;
    fn random_person(&mut self) -> Option<PersonId>;
    fn random_infected_person(&mut self) -> Option<PersonId>;
    fn infect_person(&mut self, p: PersonId, t: Option<f64>);
    fn recover_person(&mut self, p: PersonId, t: f64);
    fn next_event(&mut self);
    fn setup(&mut self);
}

impl InfectionLoop for Context {
    fn get_params(&self) -> &Parameters {
        self.get_global_property_value(Params).unwrap()
    }
    fn get_options(&self) -> &ModelOptions {
        self.get_global_property_value(Options).unwrap()
    }
    fn get_stats(&self) -> &ModelStats {
        self.get_data(ModelStatsPlugin)
    }
    fn infected_people(&self) -> usize {
        if self.get_options().queries_enabled {
            #[allow(deprecated)]
            self.query_people((InfectionStatus, InfectionStatusValue::Infectious))
                .len()
        } else {
            self.get_data(NonQueryInfectionTracker).len()
        }
    }
    fn random_person(&mut self) -> Option<PersonId> {
        self.sample_person(NextPersonRng, ())
    }
    fn random_infected_person(&mut self) -> Option<PersonId> {
        if self.get_options().queries_enabled {
            self.sample_person(
                NextPersonRng,
                (InfectionStatus, InfectionStatusValue::Infectious),
            )
        } else {
            let infected = self.get_data(NonQueryInfectionTracker);
            if infected.is_empty() {
                None
            } else {
                let index = self.sample_range(NextPersonRng, 0..infected.len());
                Some(infected[index])
            }
        }
    }
    fn infect_person(&mut self, p: PersonId, t: Option<f64>) {
        if self.get_person_property(p, InfectionStatus) != InfectionStatusValue::Susceptible {
            return;
        }

        self.set_person_property(p, InfectionStatus, InfectionStatusValue::Infectious);

        // Only record incidence if there is a time (otherwise, this is during setup)
        if let Some(current_t) = t {
            let stats_data = self.get_data_mut(ModelStatsPlugin);
            stats_data.record_infection(current_t);
        }

        // Update the non-query index
        self.get_data_mut(NonQueryInfectionTracker).insert(p);
    }

    fn recover_person(&mut self, p: PersonId, _t: f64) {
        self.set_person_property(p, InfectionStatus, InfectionStatusValue::Recovered);

        let stats_data = self.get_data_mut(ModelStatsPlugin);
        stats_data.record_recovery();

        // Update the non-query index
        self.get_data_mut(NonQueryInfectionTracker)
            .retain(|&x| x != p);
    }
    fn next_event(&mut self) {
        let params = self.get_params();
        let infection_rate = params.r0 / params.infectious_period;
        let n = self.infected_people() as f64;

        // If there are no more infected people, exit the loop.
        if n == 0.0 {
            return;
        }

        let infection_event_rate = infection_rate * n;
        let recovery_event_rate = n / params.infectious_period;

        let infection_event_time =
            self.sample_distr(NextEventRng, Exp::new(infection_event_rate).unwrap());
        let recovery_event_time =
            self.sample_distr(NextEventRng, Exp::new(recovery_event_rate).unwrap());

        let p = self.random_person().unwrap();
        if infection_event_time < recovery_event_time {
            if self.get_person_property(p, InfectionStatus) == InfectionStatusValue::Susceptible {
                self.add_plan(
                    self.get_current_time() + infection_event_time,
                    move |context| {
                        context.infect_person(p, Some(context.get_current_time()));
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
                    context.recover_person(p, context.get_current_time());
                }
                if context.infected_people() > 0 {
                    context.next_event();
                }
            });
            return;
        }

        // If we didn't schedule any plans, retry.
        self.next_event();
    }
    fn setup(&mut self) {
        let &Parameters {
            population,
            initial_infections,
            seed,
            max_time,
            ..
        } = self.get_params();
        let &ModelOptions { queries_enabled } = self.get_options();

        self.init_random(seed);

        if queries_enabled {
            self.index_property(InfectionStatus);
        }

        // Set up population
        for _ in 0..population {
            self.add_person(()).unwrap();
        }

        // Seed infections
        for p in self.sample_people(
            NextPersonRng,
            (InfectionStatus, InfectionStatusValue::Susceptible),
            initial_infections,
        ) {
            self.infect_person(p, None);
        }

        self.add_plan(max_time, |context| {
            context.shutdown();
        });

        assert_eq!(
            self.infected_people(),
            initial_infections,
            "should have infected people at start"
        );
        assert_eq!(
            self.get_stats().get_current_infected(),
            initial_infections,
            "stats should be initialized with initial infections"
        );
    }
}

pub struct Model {
    ctx: Context,
}

impl Model {
    pub fn new(params: Parameters, options: ModelOptions) -> Self {
        let mut ctx = Context::new();
        ctx.set_global_property_value(Params, params).unwrap();
        ctx.set_global_property_value(Options, options).unwrap();
        Self { ctx }
    }
    pub fn get_stats(&self) -> &ModelStats {
        self.ctx.get_stats()
    }
    pub fn run(&mut self) {
        self.ctx.setup();
        // Set up the first event in the loop
        self.ctx.next_event();
        self.ctx.execute();
        self.ctx.get_stats().check_extinction();
    }
}

#[cfg(test)]
mod test {
    use super::super::ParametersBuilder;
    use super::*;

    use approx::assert_relative_eq;

    fn model_variants() -> Vec<Model> {
        vec![
            ModelOptions::default(),
            ModelOptions {
                queries_enabled: true,
            },
        ]
        .into_iter()
        .map(|options| Model::new(Parameters::default(), options))
        .collect()
    }

    #[test]
    fn infected_counts() {
        for mut model in model_variants() {
            model.ctx.setup();
            assert_eq!(model.ctx.infected_people(), 5);
            let p = model
                .ctx
                .sample_person(
                    NextPersonRng,
                    (InfectionStatus, InfectionStatusValue::Susceptible),
                )
                .unwrap();
            model.ctx.infect_person(p, Some(0.0));
            assert_eq!(model.ctx.infected_people(), 6);
            assert_eq!(model.ctx.get_stats().get_cum_incidence(), 1);
            model.ctx.recover_person(p, 0.0);
            assert_eq!(model.ctx.infected_people(), 5);
            assert_eq!(model.ctx.get_stats().get_cum_incidence(), 1);
        }
    }

    #[test]
    fn get_random_infected_person() {
        for mut model in model_variants() {
            model.ctx.setup();
            let p = model.ctx.random_infected_person();
            assert!(p.is_some());
        }
    }

    #[test]
    fn test_model_attack_rate() {
        let population = 10_000;
        let mut model = Model::new(
            ParametersBuilder::default()
                .population(population)
                .build()
                .unwrap(),
            ModelOptions {
                // Faster
                queries_enabled: false,
            },
        );
        model.run();

        // Final size relation is ~58%
        let incidence = model.get_stats().get_cum_incidence() as f64;
        let expected = population as f64 * 0.58;
        assert_relative_eq!(incidence, expected, max_relative = 0.02);
    }

    #[test]
    fn run_model_disable_queries() {
        use ixa::prelude::*;
        let mut no_queries = Model::new(
            Parameters::default(),
            ModelOptions {
                queries_enabled: false,
            },
        );
        let mut with_queries = Model::new(
            Parameters::default(),
            ModelOptions {
                queries_enabled: true,
            },
        );

        assert_eq!(
            no_queries.ctx.infected_people(),
            no_queries
                .ctx
                .query_people((InfectionStatus, InfectionStatusValue::Infectious))
                .len(),
            "no queries variant should compute infected people correctly",
        );

        no_queries.run();
        with_queries.run();
    }
}
