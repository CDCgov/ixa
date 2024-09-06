# Parameters loading in Ixa

The goal of global properties is to load values for parameters or variables that will be accessed all throughout the simulation and can change during simulation. An example usage is shown below.

```rust
use ixa::context::Context;
use ixa::global_properties::GlobalPropertiesContext;

mod global_properties;
mod people;

struct Parameters {
	population: usize = 10,
	max_time: f64 = 100,
}

fn main(){
	let mut context = Context::new();

	global_properties::define_global_property(Population, Parameters.population);
	global_peroperties::define_global_property(Max_Time, Parameters.max_time);

	let population_size: usize = global_properties::get_global_property(Population);
	let max_time: f64 = global_properties::get_global_property(Max_Time);


	for _ in 0..population_size {
		context.create_person();
	}
	context.add_plan(max_time, |context| {
		context.shutdown();
	});

	context.execute();

}
```
