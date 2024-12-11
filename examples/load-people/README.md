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
struct, which internally stores its automatically assigned `id` (the index in
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
pub enum RiskCategoryValue { High, Low }

define_person_property!(Age, u8);
define_person_property!(RiskCategory, RiskCategoryValue);
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


### Initializing person properties

In ixa, every property must be initialized before it is accessed. The preferred
way to do this is to define an `initializer` which is lazily evaluated,
but you can also assign initial values manually – several patterns
are described below, and demonstrated in the example model. You can do this
using the `define_person_property!` macro.

Note that when initial values are assigned they do *not* trigger a
`PersonPropertyChangeEvent`.

#### Simple default values

If you want all people to be initialized with the same default value, you can
simply pass in the value to the the `define_person_property_with_default!` macro.

You can see an example of this in `sir.rs`, which assigns a default disease
status:

```rust
#[derive(Copy)]
pub enum DiseaseStatusValue { S, I, R }
define_person_property_with_default!(DiseaseStatus, DiseaseStatusValue, DiseaseStatus::S);
```

#### Custom initializer

If you need custom logic or you have dependencies on other properties to compute
initial values, you can also define a custom initializer that takes a reference
to context and a person identifier using `define_person_property!`.

Initializers are called lazily the first time the property is accessed via
`get_person_property` and do *not* trigger change events.

For example,`vaccine.rs` defines an initializer that computes how many vaccine
doses someone should be assigned based on their age:

```rust
define_person_property!(
    VaccineDoses,
    u8,
    |context, person_id| {
        let age = context.get_person_property(person_id, Age);
        if (age > 10) { 1 } else { 0 }
    }
);
```

Sometimes properties may need to be initialized with data contributed from somewhere
else. If that's the case, you can make it available via context:

```rust
define_person_property!(
    VaccineType,
    VaccineTypeValue,
    |_context, _person_id| {
        context.get_random_vaccine()
    }
);

impl VaccineContextExt for Context {
    fn get_random_vaccine() {
        //....
    }
}
```

#### Manual assignment

You can also initialize values manually with `set_person_property`, which will
override any default initializers on the type. However, you must be careful to
ensure that this happens before the property is accessed (or the simulation will panic).

One common use case for manual assignment is when you need to load people from a csv file.
In that case, you can read the properties and assign them a population loader.

You can see an example of this in `population_loader.rs`:

```rust
let person = context.add_person();
context.set_person_property(person, Age, record.age);
context.set_person_property(person, RiskCategory, record.risk_category);
let (vaccine_efficacy, vaccine_type) = context.generate_vaccine_props(record.risk_category);
context.set_person_property(person, VaccineEfficacy, vaccine_efficacy);
context.set_person_property(person, VaccineType, vaccine_type);
```

As long as you assign properties in the same function context as where add_person
is created, they will be assigned before any other regular `PersonCreated` events
handlers are called.

### Observing person property changes

Models can subscribe to a `PersonPropertyChangeEvent` in order to observe when
a person property is changed, such as to output the change to a report. Note
that when properties are first set, they will *not* emit any change events;

```rust
 context.subscribe_to_event(
        |_context, event: PersonPropertyChangeEvent<DiseaseStatus>| {
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
