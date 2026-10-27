//! Households: people are created in households of a random size, and each
//! transmission attempt finds them either at home or in the community.
use ixa::core_epi::{Person, PersonId};
use ixa::entity::entity_set::EntitySet;
use ixa::prelude::*;

use crate::parameters::ParametersExt;

/// Where a transmission attempt finds a person.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Setting {
    Home,
    Community,
}

define_rng!(HouseholdRng);

define_property!(struct Household(usize), Person);

/// Create the population, filling households one at a time with a size drawn
/// from the `household_sizes` parameter (the last household may be smaller).
pub fn create_households(context: &mut Context) {
    let parameters = context.get_parameters();
    let population = parameters.population;
    let sizes = parameters.household_sizes.clone();
    let weights: Vec<f64> = sizes.iter().map(|&(_, p)| p).collect();
    let mut created = 0;
    let mut household = 0;
    while created < population {
        let (size, _) = sizes[context.sample_weighted(HouseholdRng, &weights)];
        let size = size.min(population - created);
        for _ in 0..size {
            context
                .add_entity(with!(Person, Household(household)))
                .unwrap();
        }
        created += size;
        household += 1;
    }
    context.index_property::<Person, Household>();
}

impl Setting {
    /// Draw where an attempt finds a person.
    pub fn draw(context: &Context) -> Self {
        if context.sample_bool(HouseholdRng, context.get_parameters().household_probability) {
            Self::Home
        } else {
            Self::Community
        }
    }

    /// Everyone in `person`'s instance of this setting, `person` included.
    fn members<'a>(self, context: &'a Context, person: PersonId) -> EntitySet<'a, Person> {
        match self {
            Self::Home => {
                let household: Household = context.get_property(person);
                context.query(with!(Person, household))
            }
            Self::Community => context.query(Person),
        }
    }

    /// Size of `person`'s instance of this setting.
    pub fn n(self, context: &Context, person: PersonId) -> usize {
        match self {
            Self::Home => {
                let household: Household = context.get_property(person);
                context.query_entity_count(with!(Person, household))
            }
            Self::Community => context.get_entity_count::<Person>(),
        }
    }

    /// The setting's density exponent.
    fn alpha(self, context: &Context) -> f64 {
        let parameters = context.get_parameters();
        match self {
            Self::Home => parameters.household_alpha,
            Self::Community => parameters.community_alpha,
        }
    }

    /// Factor on `person`'s intrinsic infectiousness here: `(n - 1)^alpha`.
    pub fn multiplier(self, context: &Context, person: PersonId) -> f64 {
        let alpha = self.alpha(context);
        if alpha == 0.0 {
            return 1.0;
        }
        let others = (self.n(context, person) - 1) as f64;
        if alpha == 1.0 {
            others
        } else {
            others.powf(alpha)
        }
    }

    /// The larger of `person`'s setting multipliers.
    pub fn max_multiplier(context: &Context, person: PersonId) -> f64 {
        Self::Home
            .multiplier(context, person)
            .max(Self::Community.multiplier(context, person))
    }

    /// A random member other than `person`, or `None` if there is nobody.
    pub fn sample_contact(self, context: &Context, person: PersonId) -> Option<PersonId> {
        let members = self.members(context, person);
        context.sample(HouseholdRng, |rng| {
            members.sample_entity_excluding(rng, person)
        })
    }
}
