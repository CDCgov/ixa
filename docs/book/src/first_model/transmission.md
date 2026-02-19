# The Transmission Manager

We call the module in charge of initiating new infections the transmission
manager. Create the file `src/transmission_manager.rs` and add
`mod transmission_manager;` to the top of `src/main.rs` right next to the
`mod people;` statement. We need to flesh out this skeleton.

```rust
// transmission_manager.rs
use ixa::Context;

fn attempt_infection(context: &mut Context) {
  // attempt an infection...
}

pub fn init(context: &mut Context) {
  trace!("Initializing transmission manager");
  // initialize the transmission manager...
}
```

## Constants

Recall our abstract model: We assume that each susceptible person has a constant
risk of becoming infected over time, independent of past infections, expressed
as a force of infection.

There are at least three ways to implement this model:

1. At the start of the simulation, schedule each person's infection. This approach
   is possible because, in this model, everyone will eventually be infected, and all
   infections occur independently of one another.
2. At the start of the simulation, schedule a single infection. When that infection occurs,
   schedule the next infection. If, for each susceptible person, the time to infection
   is exponentially distributed, then the time until the next infection of _any_
   susceptible person in the simulation is also exponentially distributed, with a rate
   equal to the force of infection times the number of susceptibles. Upon any one
   infection, we select the next infectee at random from the remaining susceptibles
   and schedule their infection.
3. Schedule infection _attempts_, occurring at a rate equal to the force of infection
   times the total number of people. Upon any one infection attempt, we check if the
   attempted infectee is susceptible, and, if so, infect them. We then select the next
   attempted infectee at random from the entire population, and schedule their attempted
   infection. Infection attempts occur at a rate equal to the force of infection times
   the total number of people.

These three approaches are mathematically equivalent. Here we demonstrate the third
approach because it is the simplest to implement in ixa.

We have already dealt with constants when we defined the constant `POPULATION`
in `main.rs`. Let's define `FORCE_OF_INFECTION` right next to it. We also cap
the simulation time to an arbitrarily large number, a good practice that
prevents the simulation from running forever in case we make a programming
error.

```rust
// main.rs
{{#rustdoc_include ../../models/disease_model/src/main.rs:header}}
// ...the rest of the file...
```

## Infection Attempts

We need to import these constants into `transmission_manager`. To define a new
random number source in Ixa, we use `define_rng!`. There are other symbols from
Ixa we will need for the implementation of `attempt_infection()`. You can have
your IDE add these imports for you as you go, or you can add them yourself now.

```rust
// transmission_manager.rs
{{#rustdoc_include ../../models/disease_model/src/transmission_manager.rs:imports}}
// ...the rest of the file...
```

The function `attempt_infection()` needs to do the following:

1. Randomly sample a person from the population to attempt to infect.
2. Check the sampled person's _current_ `InfectionStatus`, changing it to
   infected (`InfectionStatus::I`) if and only if the person is currently
   susceptible (`InfectionStatus::S`).
3. Schedule the next infection attempt by inserting a plan into the timeline
   that will run `attempt_infection()` again.

```rust
{{#rustdoc_include ../../models/disease_model/src/transmission_manager.rs:attempt_infection}}
```

Read through this implementation and make sure you understand how it
accomplishes the three tasks above. A few observations:

- The method call `context.sample_entity(TransmissionRng, ())` takes the name of
  a random number source and a query and returns an `Option\<PersonId>`, which
  can have the value of `Some(PersonId)` or `None`. In this case, we give it the
  "empty query" `()`, which means we want to sample from the entire population.
  The population will never be empty, so the result will never be `None`, and so
  we just call `unwrap()` on the `Some(PersonId)` value to get the `PersonId`.
- If the sampled person is not susceptible, then the only thing this function
  does is schedule the next attempt at infection.
- The time at which the next attempt is scheduled is sampled randomly from the
  exponential distribution according to our abstract model and using the random
  number source `TransmissionRng` that we defined specifically for this purpose.
- None of this code refers to the people module (except to import the types
  `InfectionStatus` and `PersonId`) or the infection manager we are about to
  write, demonstrating the software engineering principle of
  [modularity](https://en.wikipedia.org/wiki/Component-based_software_engineering).

> [!INFO] Random Number Generators
>
> Each module generally defines its own random number source with `define_rng!`,
> avoiding interfering with the random number sources used elsewhere in the
> simulation in order to preserve determinism. In Monte Carlo simulations,
> _deterministic_ pseudorandom number sequences are desirable because they
> ensure reproducibility, improve efficiency, provide control over randomness,
> enable consistent statistical testing, and reduce the likelihood of bias or
> error. These qualities are critical in scientific computing, optimization
> problems, and simulations that require precise and _verifiable_ results.
