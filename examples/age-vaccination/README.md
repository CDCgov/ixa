# Age-specific time-varying vaccination

This example incorporates vaccination in a population. Vaccination rates depend on vaccine availability (which is given by an input file) and it's only available for specific age groups. Vaccine availability depends on the region (e.g., State A, State B, etc).  This example is inspired by RSV vaccination focused on children and old adults, and the input vaccine data will look like the table below.

| Day | Vaccine ID | Number of vaccines | Region |
|-----|------------|--------------------|--------|
| 1   | 1          | 100                | A      |
| 1   | 1          | 50                 | A      |
| ... |            |                    | ...    |
| 100 | 1          | 100                | B      |
| 100 | 2          | 100                | B      |


In this model, we do not reproduce clinical manifestations of the disease and only model infected states. Vaccine efficacy reduces the probability of being infected upon an infection attempt, and it is specified for each age group (< 1yr and >65 yr) in the configuration file.

```json
{
	"vaccine": {
		id:1, ve:0.6,age_group:{0,1},
		id:2, ve:0.8, age_group:{65,200}
}
```

# Model requirements

This model builds on the basic-transmission model. It requires additional features as shown below.
- Age-specific force of infection
- Aging
- Births and deaths
- Ability to look up people based on their person properties (i.e., region and age).
- Vaccine and its protective effects
  - Vaccine reduces the probability of being infected upon an infection attempt.


# Main routine
All modules are loaded and initialized. It is important to specify whether the order of modules affects initialization. Without infection, the main parts to model are: births, aging, deaths and vaccine distribution. 

```rust
fn main() {
    let mut context = Context::new();
    	
	parameters = parameters_loader::init_parameters();
	context.init_random(parameters.seed);
	population_manager::init();
	
    transmission_manager::init(&mut context);
    infection_manager::init(&mut context);
    incidence_report::init(&mut context);

	//initialize vaccine module
	vaccine_manager::init(&mut context);

    context.add_plan(MAX_TIME, |context| {
        context.shutdown();
    });

    context.execute();
}
```

# Population manager: initialization, births, deaths, and aging

1. Births and deaths are modeled as a Poisson process with a rate defined as `birth_rate` and `death_rate`, so that the time to next birth is defined in the model as `time_to_next_birth = current_time + sample_exp(birth_rate)`. 
2. Deaths should have an age-specific hazard since it's more likely to die when a person is older. Deaths require the implementation of `sample_random_person` and `remove_person`. 
   - Deaths require an event that other modules should register to if needed to remove death people from their data bases, cancel all plans, and update population size. 

```rust 
fn schedule_birth(context: &mut Context) {
    let person = context.add_person();
    context.initialize_person_property(person, Age, 0);
    context.initialize_person_property(person, RiskCategoryType, RiskCategory::Low);

    let next_birth_event = context.get_current_time() +
        context.sample_distr(PeopleRng, Exp::new(parameters.birth_rate).unwrap());

    context.add_plan(next_birth_event,
        move |context| {
            schedule_birth(context);
    });
}
```

# Vaccine module

```rust


```

# Future improvements

- Transmission
- Immunity waning
- To ensure modularity, we require an infection manager that keeps track of interventions and how they affect infectiousness/susceptibility changes.
- Improve vaccine parameters and mode of action (delay, waning, different protections, etc)
