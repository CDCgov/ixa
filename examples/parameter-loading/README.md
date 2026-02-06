# Parameters loading in Ixa

The goal of global properties is to load values for parameters or variables that
will be accessed all throughout the simulation. For this example, we will build
on the basic-infection example and focus on parameters that can't change over
time and are read from a config file.

```yaml
population: 1000
seed: 123
foi: 0.1
infection_duration: 5.0
```

To read parameters, we create a struct called Parameters and read from the
configuration file.

```rust
pub struct ParametersValues {
    population: usize,
    max_time: f64,
    seed: u64,
    foi: f64,
    infection_duration:f64,
}
```

Parameters are read using a `parameters_loader.rs` module which loads the values
from a config file and sets the parameters as a global property, which can be
accessed by the other modules.

```rust
fn main() {
    let mut context = Context::new();

    define_global_property!(Parameters, ParametersValues);
    let p = context
        .load_parameters_from_json::<ParametersValues>("input.json")
        .unwrap();
    context.set_global_property_value(Parameters, p).unwrap();

    let parameters = context.get_global_property_value(Parameters);

    context.add_plan(parameters.max_time, |context| {
        context.shutdown();
    });
    print!("{:?}", parameters);
    context.execute();
}
```
