# Example: People and Person Properties

This example demonstrates the following features related to people and their
related properties:

* Adding people to a simulation
* Defining person properties and initializing them
* Loading a population of people and their properties from a csv file
* Assigning person properties from an external module, including assigning initial (default) values
* Handling person creation and change events

### People

At a high level, each person in an ixa simulation is represented by `u8` in the
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

For example, this model implements Age, RiskStatus, and DiseaseStatus using
`the define_person_property!` macro:

```rust
#[derive(Copy)]
pub enum RiskCategory { High, Low }

#[derive(Copy)]
pub enum DiseaseStatus { S, I, R }

define_person_property!(DiseaseStatusType, DiseaseStatus);
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

### Loading people

The `population_loader` module demonstrates a common pattern of loading/parsing people from a csv file,
where each row represents a person and has some basic properties (an age and risk status), an
adds them to the simulation.

In order to assign a base set of properties (in this case, from the csv file),
we use the `context.before_person_added` method:

```rust
context.before_person_added(move |context, person_id| {
    context.set_person_property(person_id, Age, record.age);
    context.set_person_property(person_id, RiskCategoryType, record.risk_category);
});

let _person = context.add_person();
```

Internally, this uses the immediate events system provided by ixa to ensure
callbacks are called *immediately* after the person is added, and before any
regular `PersonCreated` events handlers are called.

### Assigning additional properties from other modules

Other modules outside the population loader can also call `context.before_person_added`
to initialize their own properties. In this example, the `sir` module uses a helper method
(which internally calls `before_person_added`) to assign an initial `DiseaseStatus` state:

```rust
context.set_person_property_default_value(DiseaseStatusType, DiseaseStatus::S);
```

It is important to ensure that modules which set up person properties are initialized
*before* the population_loader:

```rust
  // First set up initialization for DiseaseStatus
  sir::init(&mut context);
  // Now add the people
  population_loader::init(&mut context)
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

### Handling uninitialized/initialized properties

If you want to have a property that starts in some kind of "uninitialized" state
and handle when it becomes initialized, we recommend that you
