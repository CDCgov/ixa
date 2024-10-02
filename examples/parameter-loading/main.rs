use ixa::define_global_property;
use ixa::{context::Context, global_properties::ContextGlobalPropertiesExt};
//use ixa::global_properties::GlobalPropertiesContext;
use serde::{Deserialize, Serialize};

// First, let's assume we already read the parameters in the Parameters struct
#[derive(Serialize, Deserialize, Debug)]
pub struct Parameters {
    population: usize,
    max_time: f64,
}

fn main() {
    let mut context = Context::new();
    let parameters = Parameters {
        population: 10,
        max_time: 20.0,
    };

    define_global_property!(Population, usize);

    context.set_global_property_value(Population, parameters.population);

    // context.define_global_property::<Population>()
    // global_properties::define_global_property(Population, Parameters.population);
    // global_peroperties::define_global_property(Max_Time, Parameters.max_time);

    // let population_size: usize = global_properties::get_global_property(Population);
    // let max_time: f64 = global_properties::get_global_property(Max_Time);

    // for _ in 0..population_size {
    //     context.create_person();
    // }
    context.add_plan(parameters.max_time, |context| {
        context.shutdown();
    });
    print!("{:?}", parameters);
    context.execute();
}
