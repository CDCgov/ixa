# The People Module

In Ixa we organize our models into _modules_ each of which is responsible for a
single aspect of the model.

> [!INFO] Modules
>
> In fact, the code of Ixa itself is organized into modules in just the same way
> models are.

Ixa is a framework for developing _agent_-based models. In most of our models,
the agents will represent people. So let's create a module that is responsible
for people and their properties—the data that is attached to each person. Create
a new file in the `src` directory called `people.rs`.

## Defining an Entity and Property

```rust
// people.rs

{{#rustdoc_include ../../models/disease_model/src/people.rs:define_property}}
```

We have to define the `Person` entity before we can associate properties with
it. The `define_entity!(Person)` macro invocation automatically defines the
`Person` type, implements the `Entity` trait for `Person`, and creates the type
alias `PersonId = EntityId\<Person>`, which is the type we can use to represent
specific instances of our entity, a single person, in our simulation.

To each person we will associate a value of the enum (short for “enumeration”)
named `InfectionStatus`. An enum is a way to create a type that can be one of
several predefined values. Here, we have three values:

- **S**: Represents someone who is susceptible to infection.
- **I**: Represents someone who is currently infected.
- **R**: Represents someone who has recovered.

Each value in the enum corresponds to a stage in our simple model. The enum
value for a person's `InfectionStatus` property will refer to an individual’s
health status in our simulation.

## The module's `init()` function

While not strictly enforced by Ixa, the general formula for an Ixa module is:

1. "public" data types and functions
2. "private" data types and functions

The `init()` function is how your module will insert any data into the context
and set up whatever initial conditions it requires before the simulation begins.
For our `people` module, the `init()` function just inserts people into the
`Context`.

```rust
// Populates the "world" with people.
pub fn init(context: &mut Context) {
   trace!("Initializing people");

   for _ in 0..100 {
      let _ = context.add_entity(Person).expect("failed to add person");
   }
}
```

We use `Person` here to represent a new entity with all default property values–
our one and only `Property` was defined to have a default value of
`InfectionStatus::S` (susceptible), so no additional information is needed.

The `.expect("failed to add person")` method call handles the case where adding
a person could fail. We could intercept that failure if we wanted, but in this
simple case we will just let the program crash with a message about the reason:
"failed to add person".

The `Context::add_entity` method returns an entity ID wrapped in a `Result`,
which the `expect` method unwraps. We can use this ID if we need to refer to
this newly created person. Since we don't need it, we assign the value to the
special "don't care" variable `_` (underscore), which just throws the value
away.

## Constants

Having "magic numbers" embedded in your code, such as the constant `100` here
representing the total number of people in our model, is **_bad practice_**.
What if we want to change this value later? Will we even be able to find it in
all of our source code? Ixa has a formal mechanism for managing these kinds of
model parameters, but for now we will just define a "static constant" near the
top of `src/main.rs` named `POPULATION` and replace the literal `100` with
`POPULATION`:

```rust
{{#rustdoc_include ../../models/disease_model/src/people.rs:init}}
```

Let's revisit `src/main.rs`:

```rust
{{#rustdoc_include ../../models/disease_model/src/main.rs}}
```

1. Your IDE might have added the `mod people;` line for you. If not, add it now.
   It tells the compiler that the `people` module is attached to the `main`
   module (that is, `main.rs`).
2. We also need to declare our static constant for the total number of people.
3. We need to initialize the people module.

## Imports

Turning back to `src/people.rs`, your IDE might have been complaining to you
about not being able to find things "in this scope"—or, if you are lucky, your
IDE was smart enough to import the symbols you need at the top of the file
automatically. The issue is that the compiler needs to know where externally
defined items are coming from, so we need to have `use` statements at the top of
the file to import those items. Here is the complete `src/people.rs` file:

```rust
{{#rustdoc_include ../../models/disease_model/src/people.rs}}
```
