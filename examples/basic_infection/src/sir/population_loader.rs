use eosim::{
    context::{Component, Context},
    global_properties::GlobalPropertyContext,
    people::PeopleContext,
};

use super::global_properties::Population;

pub struct PopulationLoader {}

impl Component for PopulationLoader {
    fn init(context: &mut Context) {
        // Add people to the simulation
        let population = context
            .get_global_property_value::<Population>()
            .expect("Population not specified");
        for _ in 0..*population {
            context.add_person().execute();
        }
    }
}
