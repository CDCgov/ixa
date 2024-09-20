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

```toml
- vaccine:
	- id: 1
	- ve: 0.6
	- age_group: 0-1
- vaccine:
	- id: 2
	- ve: 0.8
	- age_group: 65+
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
All modules are loaded and initialized. It is important to specify whether the order of modules affects initialization.

```rust
fn main() {
    let mut context = Context::new();

    context.init_random(SEED);

    for _ in 0..POPULATION {
        context.create_person();
    }

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

# Vaccine module
The main

```rust
pub fn init(context: &mut Context) {
    context.subscribe_to_event::<InfectionStatusEvent>(move |context, event| {
        handle_infection_status_change(context, event);
    });
}
```

# Future improvements

- Transmission
- Immunity waning
- To ensure modularity, we require an infection manager that keeps track of interventions and how they affect infectiousness/susceptibility changes.
- Improve vaccine parameters and mode of action (delay, waning, different protections, etc)
