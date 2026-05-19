use ixa::prelude::*;
use ixa::runner::run_with_args;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct RunnerPropertyType {
    field_int: u32,
}

define_global_property!(RunnerProperty, RunnerPropertyType);

fn main() {
    run_with_args(|context, _args, _| {
        let property = context
            .get_global_property_value(RunnerProperty)
            .ok_or_else(|| IxaError::PropertyNotSet {
                name: "RunnerProperty".to_string(),
            })?;
        println!("{}", property.field_int);
        Ok(())
    })
    .unwrap();
}
