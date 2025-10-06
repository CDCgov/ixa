# Birth and death processes

This example describes the process of loading a population, adding new births
and deaths. It has a simple disease model that represents infections from a
constant force of infection that differs by age category (0,1,65) .

## Features

- Adding newborns to the simulation with some default person properties,
- Age-Groups that update with time,
- Person look-up based on their properties, or Age-Groups,
- Deaths and plan removal to ensure that dead agents are not executing plans.

## Simulation overview

This simulation builds on the basic infection example. Mainly, susceptible
individuals are exposed to a pathogen at a risk determined by a constant force
of infection, which varies by age group. Three age groups are defined. Namely,
`Newborns (< 1yr)`, `General (1-65yr)`, and `OldAdult( 65+yr)`. At
initialization, people are added to the simulation with an age from 0 - 100 yrs
and assigned one of the age groups. However, membership to these groups can
change over time as people age. People can also be added to the simulation as
newborns, as they can die at any point during the simulation.

The exposure process is initialized by scheduling an initial infection attempt
for each age group. This depends on the force of infection defined for each age
group, and the current size of the group, which only includes people who are
alive. Individuals who are infected by the pathogen may recover based on a
recovery period (`t + infection_period`) specified at initialization. These
recovery times are scheduled at the time of infection as a `plan`. The `id` for
this plan is saved in a data structure that holds the current plans for recovery
scheduled for each individual. Once the person recovers, these plans are removed
from the data structure. However, if the person dies before recovering, these
plans are canceled at the time of death. The infection status of recovered
individuals remains as recovered for the rest of the simulation, which will stop
when the plan queue is empty.

## Population manager

At initialization, the population manager adds people to the simulation as
defined in the input parameters file, and initializes each person with two
person properties: `Birth` time and `Alive` status. Birth time is estimated as a
random number between 0 and -100 to represent ages from 0 to 100 years. The
method `get_person_age` (implemented as a trait extension on `context`) can
return a person's age based on their time of birth.

## Births

New people are constantly added to the simulation based on a birth rate, which
is defined by the input parameters. The creation of new individuals is performed
as any other person added to the simulation, and their person property `Birth`
is assigned to the current simulation time `context.get_current_time()`.

```rust
fn create_new_person(&mut self, birth_time: f64) -> PersonId {
    let person = self.add_person();
    self.initialize_person_property(person, Birth, birth_time);
    self.initialize_person_property(person, Alive, true);
    person
}
```

## Deaths

People are constantly removed from the simulation based on a death rate, which
is defined by the input parameters. Every time a death is scheduled to occur,
the function `attempt_death` is called, which will set person property `Alive`
to false. \*\*Plans are not directly canceled by the population manager, this is
done directly in the module that schedules the plan (e.g., `infection_manager`)
by subscribing to events related to changes in the person property `Alive`. It
is important to keep in mind that dead individuals should not be counted for the
force of infection or other transmission events.

```rust
fn attempt_death(&mut self, person_id) {
    self.set_person_property(person_id, Alive, false);
}
```

## Age Groups

Age groups are defined in the population manager as an `enum`. These groups are
determined for each person using the method `get_person_age_group`. This
function estimates the current age group based on the time of the simulation and
the time of birth. In this example, the force of infection varies for each of
the three age groups defined below. Hence, a hash map contains the force of
infection for each of these groups and is saved as a global property.

```rust
pub enum AgeGroupRisk {
    NewBorn,
    General,
    OldAdult,
}

fn get_person_age_group(&mut self, person_id: PersonId) -> AgeGroupRisk {
    let current_age = self.get_person_age(person_id);
    if current_age <= 1.0 {
        AgeGroupRisk::NewBorn
    } else if current_age <= 65.0 {
        AgeGroupRisk::General
    } else {
        AgeGroupRisk::OldAdult
    }
}
```

## Person look-up based on properties

This example implements a function to sample a random person from a group of
people with the same person property. For instance, to sample a random person
who's alive, one can filter by the Alive property
`sample_person_by_property(Alive, true)`. A similar function is implemented to
select a random person from a specific age group. For instance, to sample
someone a Newborn, one can call the function
`sample_person(AgeGroupRisk::NewBorn`.

```rust
fn sample_person_by_property<T: PersonProperty + 'static>(
    &mut self,
    property: T,
    value: T::Value,
) -> Option<PersonId>
where
     <T as PersonProperty>::Value: PartialEq,
{
    let mut people_vec = Vec::<PersonId>::new();
    for i in 0..self.get_current_population() {
        let person_id = self.get_person_id(i);
        if self.get_person_property(person_id, property) == value {
            people_vec.push(person_id);
        }
    }
    if people_vec.is_empty() {
        None
    } else {
        Some(people_vec[self.sample_range(PeopleRng, 0..people_vec.len())])
    }
}

fn sample_person(&mut self, age_group: AgeGroupRisk) -> Option<PersonId> {
    let mut people_vec = Vec::<PersonId>::new();
    for i in 0..self.get_current_population() {
        let person_id = self.get_person_id(i);
        if self.get_person_property(person_id, Alive)
            && self.get_person_age_group(person_id) == age_group
        {
            people_vec.push(person_id);
        }
    }
    if people_vec.is_empty() {
        None
    } else {
        Some(people_vec[self.sample_range(PeopleRng, 0..people_vec.len())])
    }
}
```

## Transmission & infection progression

Infections are spread throughout the population based on a constant force of
infection, which differs for age groups 0-12m, 1-65, and 65+. Given that
population changes over time in this example, the p constant force of infection
is an approximation, as opposed to a rejection sampling approach. Infection
attempts are scheduled based on each age group force of infection. To spread the
pathogen in the population, a random person is selected for each age group using
`sample_person(age_group)`, if this person is susceptible to infection.

Infected individuals are scheduled to recover based on the infection period.
These are the only type of plans that are scheduled for an individual in this
simulation. Hence, when recovery is scheduled using `context.add_plan()`, the
`plan id` is stored in a data container named `InfectionPlansPlugin`.

```rust
let plan_id = context
    .add_plan(recovery_time, move |context| {
context.set_person_property(person_id, InfectionStatus, InfectionStatusValue::R);
    })
    .clone();
let plans_data_container = context.get_data_container_mut(InfectionPlansPlugin);
plans_data_container
    .plans_map
    .entry(person_id)
    .or_default()
    .insert(plan_id.clone());

```

These plan ids are removed from the data container once the individual recovers
form infection or dies. However, if the person dies during the simulation, the
upcoming plans need to be canceled; hence, a special function is used to handle
person removal. This function cancels plans to recover and removes their ids
from the data container for the person.

```rust
fn cancel_recovery_plans(context: &mut Context, person_id: PersonId) {
    let plans_data_container = context.get_data_container_mut(InfectionPlansPlugin);
    let plans_set = plans_data_container
        .plans_map
        .get(&person_id)
        .unwrap_or(&HashSet::<plan::PlanId>::new())
        .clone();

    for plan_id in plans_set {
        context.cancel_plan(&plan_id);
    }
    remove_recovery_plan_data(context, person_id);
}
```
