use ixa::prelude::*;
use ixa::{define_entity, define_property};
use rand_distr::Exp;

use crate::reference_sir::sir_ixa::{
    ModelOptions, ModelStatsPlugin, NextEventRng, NextPersonRng, Options, Params,
};
use crate::reference_sir::{ModelStats, Parameters};

define_entity!(Person);
define_property!(
    enum InfectionStatus {
        Susceptible,
        Infectious,
        Recovered,
    },
    Person,
    default_const = InfectionStatus::Susceptible
);

pub trait InfectionLoop {
    fn get_params(&self) -> &Parameters;
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
    fn get_stats(&self) -> &ModelStats {
        self.get_data(ModelStatsPlugin)
    }
    fn infected_people(&self) -> usize {
        self.query_entity_count::<Person, _>((InfectionStatus::Infectious,))
    }
    fn random_person(&mut self) -> Option<PersonId> {
        self.sample_entity(NextPersonRng, ())
    }
    fn random_infected_person(&mut self) -> Option<PersonId> {
        self.sample_entity(NextPersonRng, (InfectionStatus::Infectious,))
    }
    fn infect_person(&mut self, p: PersonId, t: Option<f64>) {
        // if self.get_property::<_, InfectionStatus>(p) != InfectionStatus::Susceptible {
        //     return;
        // }

        self.set_property(p, InfectionStatus::Infectious);

        // Only record incidence if there is a time (otherwise, this is during setup)
        if let Some(current_t) = t {
            let stats_data = self.get_data_mut(ModelStatsPlugin);
            stats_data.record_infection(current_t);
        }
    }

    fn recover_person(&mut self, p: PersonId, _t: f64) {
        debug_assert_eq!(
            self.get_property::<_, InfectionStatus>(p),
            InfectionStatus::Infectious
        );
        self.set_property(p, InfectionStatus::Recovered);
        debug_assert_eq!(
            self.get_property::<_, InfectionStatus>(p),
            InfectionStatus::Recovered
        );

        let stats_data = self.get_data_mut(ModelStatsPlugin);
        stats_data.record_recovery();
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

        if infection_event_time < recovery_event_time {
            let p = self.random_person().unwrap();
            if self.get_property::<_, InfectionStatus>(p) == InfectionStatus::Susceptible {
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
        self.init_random(seed);

        self.index_property::<Person, InfectionStatus>();

        // Set up population
        for _ in 0..population {
            self.add_entity::<Person, _>(()).unwrap();
        }

        // Seed infections
        let sampled_entities: Vec<PersonId> = self.sample_entities(
            NextPersonRng,
            (InfectionStatus::Susceptible,),
            initial_infections,
        );
        for p in sampled_entities {
            self.infect_person(p, None);
        }

        self.add_plan(max_time, |context| {
            context.shutdown();
        });

        debug_assert_eq!(
            self.infected_people(),
            initial_infections,
            "should have infected people at start"
        );

        debug_assert_eq!(
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
        // Print final stats
        println!(
            "Cumulative incidence: {}",
            self.ctx.get_stats().get_cum_incidence()
        );
    }
}

#[cfg(test)]
mod test {
    use approx::assert_relative_eq;

    use super::*;
    use crate::reference_sir::ParametersBuilder;

    #[test]
    fn infected_counts() {
        let mut model = Model::new(Parameters::default(), ModelOptions::default());
        model.ctx.setup();
        assert_eq!(model.ctx.infected_people(), 5);
        let p = model
            .ctx
            .sample_entity(NextPersonRng, (InfectionStatus::Susceptible,))
            .unwrap();
        model.ctx.infect_person(p, Some(0.0));
        assert_eq!(model.ctx.infected_people(), 6);
        assert_eq!(model.ctx.get_stats().get_cum_incidence(), 1);
        model.ctx.recover_person(p, 0.0);
        assert_eq!(model.ctx.infected_people(), 5);
        assert_eq!(model.ctx.get_stats().get_cum_incidence(), 1);
    }

    #[test]
    fn get_random_infected_person() {
        let mut model = Model::new(Parameters::default(), ModelOptions::default());
        model.ctx.setup();
        let p = model.ctx.random_infected_person();
        assert!(p.is_some());
    }

    #[test]
    fn test_model_attack_rate() {
        let population = 10_000;
        let mut model = Model::new(
            ParametersBuilder::default()
                .population(population)
                .build()
                .unwrap(),
            ModelOptions::default(),
        );
        model.run();

        // Final size relation is ~58%
        let incidence = model.get_stats().get_cum_incidence() as f64;
        let expected = population as f64 * 0.58;
        assert_relative_eq!(incidence, expected, max_relative = 0.04);
    }
}
