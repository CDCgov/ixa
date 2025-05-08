# The People Module

In Ixa we organize our models into *modules* each of which is responsible for a single aspect of the model.

> [!INFO] Modules
> In fact, the code of Ixa itself is organized into modules in just the same way models are.

Ixa is a framework for developing *agent*-based models. In most of our models, the agents will represent people. So let's create a module that is responsible for people and their properties—the data that is attached to each person. Create a new file in the `src` directory called `people.rs`.

## `PersonProperty`

```rust
{{#rustdoc_include ../../models/disease_model/src/people.rs:5:10}}
```

To each person we will associate a value of the enum (short for “enumeration”) named `InfectionStatusValue`. An enum is a way to create a type that can be one of several predefined values. Here, we have three values:

- **S**: Represents someone who is susceptible to infection.
- **I**: Represents someone who is currently infected.
- **R**: Represents someone who has recovered (or is otherwise no longer infectious).

Each value in the enum corresponds to a stage in our simple SIR (Susceptible, Infected, Recovered) model. The enum value for a person's `InfectionStatus` property will refer to an individual’s health status in our simulation.

The attributes written above the enum declaration, such as `#[derive(Debug, Hash, Eq, PartialEq, Clone, Copy, Serialize, Deserialize)]`, automatically add useful functionality to the enum:

- **`Debug`**: Allows the value to be printed for debugging purposes.

- **`Hash, Eq, PartialEq`**: Enable the enum to be compared and used in data structures like hash maps.

- **`Clone, Copy`**: Allow the values to be duplicated easily.

- **`Serialize, Deserialize`**: Make it possible to convert the enum to and from a format that can be stored or transmitted (for example, when saving data to a CSV file).

All of our "person properties" in our models will "derive" these attributes. It is not enough to define this enum. We have to tell Ixa that it will be a `PersonProperty`:

```rust
{{#rustdoc_include ../../models/disease_model/src/people.rs:12:16}}
```

> [!NOTE] Name Tags and Values
> Notice that there are two types associated to infection status, `InfectionStatus` and `InfectionStatusValue`. The first, `InfectionStatus`, is a tag that we will use to fetch and store values of the property, while `InfectionStatusValue` is the type of the values themselves.
> [!INFO] Default or No Default

## The module's `init()` function

While not strictly enforced by Ixa, the general formula for an Ixa module is:

 1. "public" data types and functions
 2. "private" data types and functions

The `init()` function is how your module will insert any data into the context and setup whatever initial conditions it requires before the simulation begins. For our `people` module, the `init()` function just inserts people into the `Context`.

```rust
/// Populates the "world" with people.
pub fn init(context: &mut Context) {

    trace!("Initializing people");

    for _ in 0..1000 {

        context.add_person(()).expect("failed to add person");

    }
}
```

The `context.add_person()` method call might look a little odd, because we are not giving `context` any data to insert, but that is because our one and only `PersonProperty` was defined to have a default value of `InfectionStatusValue::S` (susceptible)—so `context.add_person()` doesn't need any information to create a new person. Another odd thing is the `.expect("failed to add person")` method call. In more complicated scenarios adding a person can fail. We can intercept that failure if we wanted, but in this simple case we will just let the program crash with a message about the reason: "failed to add person".

## Constants

Having "magic numbers" embedded in your code, such as the constant `1000` here representing the total number of people in our model, is ***bad practice***. What if we want to change this value later? Will we even be able to find it in all of our source code? Ixa has a formal mechanism for managing these kinds of model parameters, but for now we will just define a "static constant" near the top of `src/main.rs` named `POPULATION` and replace the literal `1000` with `POPULATION`:

```rust
{{#rustdoc_include ../../models/disease_model/src/people.rs:18:24}}
```

Let's revisit `src/main.rs`:

```rust
// main.rs
mod people;

use ixa::prelude::*;

static POPULATION: u64 = 1000;

fn main() {
    let result =
        run_with_args(|_context: &mut Context, _args, _| {
            trace!("Initializing disease_model");
            people::init(context);
            Ok(())
        });

    match result {
        Ok(_) => {
            info!("Simulation finished executing");
        }
        Err(e) => {
            error!("Simulation exited with error: {}", e);
        }
    }
}
```

1. Your IDE might have added the `mod people;` line for you. If not, add it now. It tells the compiler that the `people` module is attached to the `main` module (that is, `main.rs`).
2. We also need to declare our static constant for the total number of people.
3. We need to initialize the people module.

## Imports

Turning back to `src/people.rs`, your IDE might have been complaining to you about not being able to find things "in this scope"—or, if you are lucky, your IDE was smart enough to import the symbols you need at the top of the file automatically. The issue is that the compiler needs to know where externally defined items are coming from, so we need to have `use` statements at the top of the file to import those items. Here is the complete `src/people.rs` file:

```rust
//people.rs
{{#rustdoc_include ../../models/disease_model/src/people.rs}}
```
