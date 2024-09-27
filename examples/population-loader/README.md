# Population loader
This example shows how to create a new population in Ixa using a `person_properties` module. Every person in the population has the following properties:

 - age: 0 - 100
 - gender: F, M
 - region: RegionA, RegionB
 - `high_risk_status`: True, False (Age-dependent)
 - `infection_status`: S, I, R

We will use a table to load the population with age, gender, and region, where each row represents a person. Infection status is by default Susceptible (S), and `high_risk_status` depends on the age of the person (0 - 20 => False, 20+ => True). The input table will look like the table bellow.


| Age | Gender | Region  |
|-----|--------|---------|
| 6   | M      | RegionA |
| 26  | F      | RegionB |
| 12  | M      | RegionA |
| 1   | F      | RegionB |

## main.rs
```rust
use ixa::context::Context;
use ixa::random::ContextRandomExt;

mod person_properties;
mod population_loader;


fn main() {
    let mut context = Context::new();

	population_loader::init(&mut context);

    context.execute();
}
```

## population_loader.rs

```rust
use ixa::Region;
use ixa::PersonProperties;

pub enum InfectionStatus {
	S,
	I,
	R,
}

struct PeopleTable {
	Age: u64,
	Gender: String,
	Region: Region
}

let people_data = array<PeopleTable>(
	{Age: 56, Gender: M, Region: RegionA},
	{Age: 26, Gender: F, Region: RegionB},
	{Age: 12, Gender: M, Region: RegionA},
	{Age: 1, Gender: F, Region: RegionB}
);

define_person_properties!(
	(age, u64),
	(gender, string),
	(region, Region),
	(high_risk_status, Boolean),
	(infection_status, InfectionStatus, InfectionStatus::S)
);


// For each person, read age, gender, and region. InfectionStatus defaults to S,
// and risk status depends on age
for person_data in people_data {
	person_builder = context.create_person();
	person_builder
		.set_person_property(age, person_data.Age)
		.set_person_property(gender, person_data.Gender);

	person_builder.add_region(person_data.Region);

	let risk_flag = true if age >= 20 else false;
	person_builder.set_person_property(high_risk_status, risk_flag);
	person_builder.build();
}

```
