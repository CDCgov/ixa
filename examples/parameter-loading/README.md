# Parameters loading in Ixa

The goal of global properties is to load values for parameters or variables that will be accessed all throughout the simulation. For this example, we will build on the basic-infection example and focus on parameters that can't change over time and are read from a config file.

```yaml
population: 1000
seed: 123
foi: 0.1
infection_duration: 5.0
```

To read parameters, we create a struct called Parameters and read from the configuration file.

```rust
use ixa::context::Context;
use ixa::global_properties::GlobalPropertiesContext;

mod global_properties;
mod people;
pub struct ParametersValues {
    population: usize,
    max_time: f64,
	seed: u64,
	foi: f64,
	infection_duration:f64,
}
```
Parameters are read using a `load-parameters.rs` module which implements the method `load_parameters_from_config` and sets the parameters as a global property, which can be accessed by the other modules.

```rust
fn main() {
    let mut context = Context::new();

    define_global_property!(Parameters, ParametersValues);
	context.load_parameters_from_config(ParameterValues, "config.yaml");

	let parameters = context.get_global_property_value(Parameters);

    context.add_plan(parameters.max_time, |context| {
        context.shutdown();
    });
    print!("{:?}", parameters);
    context.execute();
}
```
