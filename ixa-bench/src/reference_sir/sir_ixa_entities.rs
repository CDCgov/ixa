use super::{ModelStats, Parameters};
use indexmap::IndexSet;
use ixa::{
    define_entity, define_entity_property,
    entity::{EntityContextExt, EntityId},
    entity_property_for,
    prelude::*,
};
use rand_distr::Exp;

define_global_property!(Params2, Parameters);

define_entity!(pub struct Person);
define_entity_property!(
    pub enum InfectionStatus {
        Susceptible,
        Infectious,
        Recovered,
    }
);
entity_property_for!(Person => InfectionStatus, default = InfectionStatus::Susceptible);

type PersonId = EntityId<Person>;

define_rng!(PersonRng);
define_rng!(EventRng);

define_data_plugin!(ModelStatsPlugin, ModelStats, |context| {
    let params = context.get_global_property_value(Params2).unwrap();
    ModelStats::new(params.initial_infections, params.population, 0.2)
});
define_data_plugin!(
    NonQueryInfectionTracker,
    IndexSet<PersonId>,
    IndexSet::new()
);

trait InfectionLoop {
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
        self.get_global_property_value(Params2).unwrap()
    }
    fn get_stats(&self) -> &ModelStats {
        self.get_data(ModelStatsPlugin)
    }
    fn infected_people(&self) -> usize {
        self.get_data(NonQueryInfectionTracker).len()
    }
    fn random_person(&mut self) -> Option<PersonId> {
        let population = self.entity_count::<Person>();
        if population == 0 {
            return None;
        }

        let index = self.sample_range(PersonRng, 0..population);
        Some(PersonId::new(index))
    }
    fn random_infected_person(&mut self) -> Option<PersonId> {
        let infected = self.get_data(NonQueryInfectionTracker);
        if infected.is_empty() {
            None
        } else {
            let index = self.sample_range(PersonRng, 0..infected.len());
            infected.get_index(index).copied()
        }
    }
    fn infect_person(&mut self, p: PersonId, t: Option<f64>) {
        if self.get_property::<Person, InfectionStatus>(p) != InfectionStatus::Susceptible {
            return;
        }

        self.set_property::<Person, InfectionStatus>(p, InfectionStatus::Infectious);

        // Only record incidence if there is a time (otherwise, this is during setup)
        if let Some(current_t) = t {
            let stats_data = self.get_data_mut(ModelStatsPlugin);
            stats_data.record_infection(current_t);
        }

        // Update the non-query index
        self.get_data_mut(NonQueryInfectionTracker).insert(p);
    }

    fn recover_person(&mut self, p: PersonId, _t: f64) {
        self.set_property::<Person, InfectionStatus>(p, InfectionStatus::Recovered);

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
            self.sample_distr(EventRng, Exp::new(infection_event_rate).unwrap());
        let recovery_event_time =
            self.sample_distr(EventRng, Exp::new(recovery_event_rate).unwrap());

        let p = self.random_person().unwrap();
        if infection_event_time < recovery_event_time {
            if self.get_property::<Person, InfectionStatus>(p) == InfectionStatus::Susceptible {
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

        // Set up population
        for _ in 0..population {
            self.add_entity::<Person>();
        }

        // Seed infections
        let mut candidates: Vec<PersonId> = self.entity_iter::<Person>().collect();
        let seeds = initial_infections.min(candidates.len());
        for _ in 0..seeds {
            let index = self.sample_range(PersonRng, 0..candidates.len());
            let person = candidates.swap_remove(index);
            self.infect_person(person, None);
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
    pub fn new(params: Parameters) -> Self {
        let mut ctx = Context::new();
        ctx.set_global_property_value(Params2, params).unwrap();
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
    use ixa::entity::EntityContextExt;

    #[test]
    fn infected_counts() {
        let mut model = Model::new(
            ParametersBuilder::default()
                .population(10)
                .initial_infections(5)
                .build()
                .unwrap(),
        );
        model.ctx.setup();
        assert_eq!(model.ctx.infected_people(), 5);
        let p = {
            let ctx_ref = &model.ctx;
            ctx_ref
                .entity_iter::<Person>()
                .find(|&person| {
                    ctx_ref.get_property::<Person, InfectionStatus>(person)
                        == InfectionStatus::Susceptible
                })
                .unwrap()
        };
        model.ctx.infect_person(p, Some(0.0));
        assert_eq!(model.ctx.infected_people(), 6);
        assert_eq!(model.ctx.get_stats().get_cum_incidence(), 1);
        model.ctx.recover_person(p, 0.0);
        assert_eq!(model.ctx.infected_people(), 5);
        assert_eq!(model.ctx.get_stats().get_cum_incidence(), 1);
    }

    #[test]
    fn get_random_infected_person() {
        let population = 10_000;
        let mut model = Model::new(
            ParametersBuilder::default()
                .population(population)
                .build()
                .unwrap(),
        );
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
        );
        model.run();

        // Final size relation is ~58%
        let incidence = model.get_stats().get_cum_incidence() as f64;
        let expected = population as f64 * 0.58;
        assert_relative_eq!(incidence, expected, max_relative = 0.02);
    }
}
