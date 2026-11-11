mod parameters;

use ixa::prelude::*;

use crate::parameters::Parameters;

define_global_property!(Params, Parameters);

fn main() {
    let parameters = Parameters::default();
    parameters.validate().expect("invalid Parameters");

    let mut context = Context::new();
    context
        .set_global_property_value(Params, parameters)
        .expect("parameters should only be initialized once");

    let (seed, max_time) = {
        let parameters = context
            .get_global_property_value(Params)
            .expect("parameters should be initialized");
        (parameters.seed, parameters.max_time)
    };

    context.init_random(seed);
    context.add_plan(max_time, |context| {
        println!("The current time is {}", context.get_current_time());
        context.shutdown();
    });
    context.execute();
}
