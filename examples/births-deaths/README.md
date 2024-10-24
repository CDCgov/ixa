# Birth and death processes

This example describes the process of loading a population, adding new births and deaths. It has a simple sir model that represents infections from a constant force of infection that differs by age category (0,2m, 6m, 65y).

# Features

  * Adding people to the simulation with person properties that are set by default for newborns
  * Looking up people based on their current age
  * Introduces the concept of updated person properties. For instance, if a person's risk property depends on their age, it should change when they age.
  * Introduces deaths and how they are removed from the population.

# Simulation overview
The simulation loads parameters that contain a seed, population size, birth and death rates, as well as foi rates. Then, a the population manager module is initialized to set up the initial population and their person properties. This module is also in charge of scheduling new births and deaths based on the specified rates in parameters. Finally, the transmission manager module is initialized. Infection attempts are scheduled to occur based on the constant age-specific force of infection. Once an infection event is scheduled, a susceptible individual is selected to be infected. This individual is required to be alive at the time of the infection attempt. After an infection attempt is finished, the next infection event is scheduled based on a constant force of infection.

Infected individuals schedule their recovery at time `t + infection_period`. The infection status of recovered individuals remains as recovered for the rest of the simulation.


```rust main.rs
fn main() {
	let mut context = Context::new();
	let current_dir = Path::new(file!()).parent().unwrap();
	let file_path = current_dir
		.join("input.json");

	parameters_loader::init();

	let parameters = context.get_global_property_value(Parameters).clone();
	context.init_random(parameters.seed);
	population_manager::init(&mut context);
	transmission_manager::init(&mut context);
}

```

# People and person properties
When the `Population manager` module initializes, a number of persons are created and given a unique person id (from `0` to `population_size`). This functionality is provided by an `create_person` method from the `people` module in `ixa`, which adds them to a `People` data container. This function also defines a special person property that determines people's life status in the simulation `define_person_property!(Alive, bool, true)`.

The population manager also defines an infection status person property and an age property, which is assigned randomly based on a uniform distribution.

```rust
InfectionStatus = enum(
    S,
    I,
    R
);

pub enum RiskCategory {
    High,
    Low,
}

define_person_property!(Age, u8); // Age in days
define_person_property!(RiskCategoryType, RiskCategory); // This is a derived property that depends on age.
define_person_property!(InfectionStatusType, InfectionStatus, InfectionStatus::S);

for (person_id in 0..parameters.get_parameter(population_size)) {
    context.create_person(person_id = person_id)
	let age_in_days = context.sample_unif(PeopleRng, 0, 100 * 365);
	context.initialize_person_property(person, Age, age_in_days);
	let	risk_category = RiskCategory.get_risk_category(age_in_days);
    context.initialize_person_property(person, RiskCategoryType, risk_category);
}
```
## Births and deaths

### Births
Some requirements are necessary to include births in the simulation. Namely,
  * Newborns increase the population,
  * Person properties are set for newborns at the time of creation, including derived properties,
  * Newborns become available to look up and should be considered alive after their time of birth,
  * A person created events should be emitted.

### Deaths
Requirements for deaths include removing from simulation and canceling any plans for the `person_id`.
  * Deaths reduce current population, but not for the purposes of the person id for next new born. This means that a counter for total population is required as well as newborns and deaths,
  * Alive person property should be set to `false`,
  * All plans should be canceled for `person_id` when they are removed from population. This should happen inside `people.rs` so that modules aren't required to continuously observe for death events,
  * Death people should not be counted for the force of infection or other transmission events.


# Transmission manager
Infections are spread throughout the population based on a constant force of infection, which differs for age groups 0-12m, 1-65, and 65+. Infection attempts are scheduled based on each age group force of infection. This requires the implementation of an Ixa functionality to look up individuals based on their current age.

```rust transmission_manager.rs
fn schedule_infection(context, age_group, foi_age) {
    transmission_rng = rng.get_rng(id = transmission);
    population = context.get_population(age_group);
    person_to_infect = context.get_random_person(age_group);

    if (context.get_infection_status(person_to_infect) == Susceptible) {
        context.set_infection_status(person_to_infect, Infected);
    }

    time_next_infection = transmission_rng.draw_exponential(foi_age) / population;
    context.add_plan(attempt_infection(context, age_group, foi_age), time = context.get_time() + time_next_infection);
}

//initialization
init(context) {
    context.add_rng(id = transmission);
    age_groups = parameters.age_groups;
    vec_foi_age = parameters.foi_age;
    for n in range(vec_foi_age) {
        context.add_plan(attempt_infection(context, age_groups[n], vec_foi_age[n]), time = 0);
    }
}
```
