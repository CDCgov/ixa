use crate::hyperfine_group;
use ixa::entity::EntityId;
use ixa::prelude::*;
use ixa::{define_entity, define_property};
use std::any::Any;
use std::hint::black_box;

const POPULATION: usize = 2_000_000;

// Static implementation
struct BaselineContext {
    entities: usize,
    property_value: Vec<InfectionStatus>,
}
impl BaselineContext {
    fn new() -> Self {
        Self {
            entities: 0,
            property_value: Vec::new(),
        }
    }
}
fn baseline() {
    let mut context = BaselineContext::new();
    let mut ids = Vec::new();
    for id in 0..POPULATION {
        ids.push(id);
        context.entities += 1;
        context.property_value.push(InfectionStatus::Susceptible)
    }
    for index in ids {
        black_box(context.property_value.get(index));
    }
}

// Dyn implementation
struct DynContext {
    entities: usize,
    property_values: Vec<Box<dyn Any>>,
}
impl DynContext {
    fn new() -> Self {
        Self {
            entities: 0,
            property_values: Vec::new(),
        }
    }
}

fn baseline_dyn() {
    let mut context = DynContext::new();
    let mut ids = Vec::new();
    context
        .property_values
        .push(black_box(Box::new(Vec::<InfectionStatus>::new())));

    for id in 0..POPULATION {
        ids.push(id);
        let property_values = context
            .property_values
            .get_mut(0)
            .unwrap()
            .downcast_mut::<Vec<InfectionStatus>>()
            .unwrap();
        property_values.push(InfectionStatus::Susceptible);
    }

    for index in ids {
        let property_values = context
            .property_values
            .get(0)
            .unwrap()
            .downcast_ref::<Vec<InfectionStatus>>()
            .unwrap();
        black_box(property_values.get(index));
    }
}

// Ixa Entities
define_entity!(Person);
define_property!(struct Age(usize), Person);
define_property!(
    enum InfectionStatus {
        Susceptible,
        Infectious,
        Recovered,
    },
    Person,
    default_const = InfectionStatus::Susceptible
);

hyperfine_group!(
    property_access {
        // Static implementation
        baseline => {
            baseline();
        },

        baseline_dyn => {
            baseline_dyn();
        },

        entities_default => {
            let mut context = Context::new();
            // Set up population
            let mut ids: Vec<EntityId<Person>> = Vec::new();

            for _ in 0..POPULATION {
                ids.push(context.add_entity::<Person, _>((Age(12),)).unwrap());
            }
            for id in ids {
                black_box(context.get_property::<Person, InfectionStatus>(id));
            }
        },

         entities_required => {
            let mut context = Context::new();
            // Set up population
            let mut ids: Vec<EntityId<Person>> = Vec::new();

            for _ in 0..POPULATION {
                ids.push(context.add_entity::<Person, _>((Age(12),)).unwrap());
            }
            for id in ids {
                black_box(context.get_property::<Person, Age>(id));
            }
        },

    }
);
