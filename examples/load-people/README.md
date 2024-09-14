# Example: People and Person Properties

This example demonstrates the following features related to people and their
related properties:

* Adding people to a simulation
* Defining person properties and initializing them
* Loading a population of people and their properties from a csv file
* Assigning person properties from an external module, including assigning initial (default) values
* Handling person creation and change events

### People

At a high level, each person in an ixa simulation is represented by an integer in the
range of `0` to `population - 1`, where `population` is the total number of people.

When you are referring to a person in order to do things with them, you will use a `PersonId`
struct, which internally stories its automatically assigned `id` (the index in
the population range):

```rust
 let person: PersonId = context.add_person();
 println!("Person {} was created", person.id)
 ```

### Person Properties

Person properties are a pair of an identifier type and a value type, which represent
some characteristic or state about a person.

For example, this model implements Age and RiskStatus using
`the define_person_property!` macro:

```rust
#[derive(Copy)]
pub enum RiskCategory { High, Low }

define_person_property!(Age, u8);
define_person_property!(RiskCategoryType, RiskCategory);
```

Person property value types **must** implement `Copy` in order to make them efficient
to be copied around in large numbers. This generally means you need to use simple
types (e.g., floats, integers, booleans, an enum instead of a string).

Context provides methods to get and set person properties for a given person:

```rust
let person = context.add_person();
context.set_person_property(person, Age, 69);
assert!(context.get_person_property(person, Age), 69);
```

Note that if you try to access a person property that is not initialized on the
given person, the simulation will panic.

### Initializing person properties

If you wish, you can declare an initializer for for a person property, which will
lazily assign a value the first time `get_person_property` is called.

For simple default values, you can use the define_person_property macro:

```rust
#[derive(Copy)]
pub enum DiseaseStatus { S, I, R }
define_person_property!(DiseaseStatusType, DiseaseStatus, DiseaseStatus::S);
```

If you need custom logic or you have dependencies on other properties to compute
initial values, you can implement a custom initializer on your property struct.
The initializer takes a reference to context and a person identifier, and should
return an option.

```rust
pub struct VaccineDoses;
impl PersonProperty for VaccineDoses {
    type Value = u8;
    fn initialize(context: &Context, person_id: PersonId) -> Option<Self::Value> {
        let age = context.get_person_property(person_id, Age);
        if (age > 10) { Some(1) } else { Some(0) }
    }
}
```

### Loading people from files

The `population_loader` module demonstrates a common pattern of loading/parsing people from a csv file,
where each row represents a person and has some basic properties (an age and risk status), an
adds them to the simulation.

In order to assign a base set of properties (in this case, from the csv file),
we use the `context.before_person_added` method:

```rust

let person = context.add_person();
context.set_person_property(person_id, Age, record.age);
context.set_person_property(person_id, RiskCategoryType, record.risk_category);
```

As long as you assign properties in the same function context as where add_person
is created, they will be assigned before any other regular `PersonCreated` events
handlers are called.

### Loading data from other modules

Sometimes properties may need to be initialized with data contributed from another
module. If that's the case, you should expose a method as a trait extension on context
and make it available to the initializer:

```rust
struct VaccineType;
impl PersonProperty for VaccineType {
    type Value = u8;
    fn initialize(context: &Context, person: PersonId) -> Option<Self::Value> {
        let vaccine = context.get_random_vaccine();
        Some(vaccine)
    }
}

impl VaccineContextExt for Context {
    fn get_random_vaccine() {
        //....
    }
}
```

If your property doesn't have dependencies on other properties, you could
also do this in the population loader:

```rust
let person = context.add_person();
let age = context.get_person_property(Age);
let (vaccine_type, doses) = context.get_random_vaccine(age);
context.set_person_property(person, VaccineType, vaccine_type);
context.set_person_property(person, VaccineDoses, doses);
```

### Observing person property changes

Models can subscribe to a `PersonPropertyChangeEvent` in order to observe when
a person property is changed, such as to output the change to a report. Note
that when properties are first set, they will *not* emit any change events;

```rust
 context.subscribe_to_event(
        |_context, event: PersonPropertyChangeEvent<DiseaseStatusType>| {
            let person = event.person_id;
            println!(
                "Person {} changed disease status from {:?} to {:?}",
                person.id, event.previous, event.current,
            );
        },
    );
```

If you want to observe the initial value of a property, you should subscribe to
the `PersonCreatedEvent` and access the property with `context.get_person_property`.
