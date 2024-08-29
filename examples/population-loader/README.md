# Population loader
This example shows how to create a new population in Ixa using a `person_properties` module. Every person in the population has the following properties:

 - age: 0 - 100
 - gender: F, M
 - region: RegionA, RegionB
 - `high_risk_status`: True, False (Age-dependent)
 - `infection_status`: S, I, R

We will use a table to load the population with age, gender, and region, where each row represents a person. Infection status is by default Susceptible (S), and high_risk_status depends on the age of the person (0 - 20 => False, 20+ => True). The input table will look like the table bellow.

|-----|--------|---------|
| Age | Gender | Region  |
|-----|--------|---------|
| 6   | M      | RegionA |
| 26  | F      | RegionB |
| 12  | M      | RegionA |
| 1   | F      | RegionB |
|-----|--------|---------|

# Simulation
```rust
use ixa::context::Context;
use ixa::random::ContextRandomExt;

mod person_properties;

static POPULATION: u64 = 10;

struct PeopleTable {
	Age: u64,
	Gender: String,
	Region: Region
}

array people_data = array<PeopleTable>(
	{Age: 56, Gender: M, Region: RegionA},
	{Age: 26, Gender: F, Region: RegionB},
	{Age: 12, Gender: M, Region: RegionA},
	{Age: 1, Gender: F, Region: RegionB}
);

fn main() {
    let mut context = Context::new();

    for _ in 0..POPULATION {
        context.create_person();
    }


    context.execute();
}
```
