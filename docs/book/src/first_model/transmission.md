# The Transmission Manager

We call the module in charge of initiating new infections the transmission manager. Create the file `src/transmission_manager.rs` and add `mod transmission_manager;` to the top of `src/main.rs` right next to the  `mod people;` statement. We need to flesh out this skeleton.

```rust
// transmission_manager.rs
use ixa::Context;

fn attempt_infection(context: &mut Context) {

}

pub fn init(context: &mut Context) {
 trace!("Initializing transmission manager");

}
```

## Constants

Recall our abstract model: We assume that each susceptible person has a constant risk of becoming infected over time, independent of past infections, expressed as a force of infection. Mathematically, this results in an exponentially distributed duration between infection events. So we need to represent the constant `FORCE_OF_INFECTION` and a random number source to sample exponentially distributed random time durations.

We have already dealt with constants when we defined the constant `POPULATION` in `main.rs`.  Let's define `FORCE_OF_INFECTION` right next to it. We also cap the simulation time to an arbitrarily large number, a good practice that prevents the simulation from running forever in case we make a programming error.

```rust
// main.rs
mod people;
mod transmission_manager;

use ixa::Context;

static POPULATION: u64 = 1000;
static FORCE_OF_INFECTION: f64 = 0.1;
static MAX_TIME: f64 = 300.0;
// ...the rest of the file...
```

## Infection Attempts

We need to import these constants into `transmission_manager`. To define a new random number source in Ixa, we use `define_rng!`.  There are other symbols from Ixa we will need for the implementation of `attempt_infection()`. You can have your IDE add these imports for you as you go, or you can add them yourself now.

```rust
// transmission_manager.rs
{{#include ../../models/disease_model/src/transmission_manager.rs:1:10}}
// ...the rest of the file...
```

The function `attempt_infection()` needs to do the following:

1. Randomly sample a person from the population to attempt to infect.
2. Check the sampled person's *current* `InfectionStatus`, changing it to infected (`InfectionStatusValue::I`) if and only if the person is currently susceptible (`InfectionStatusValue::S`).
3. Schedule the next infection attempt by inserting a plan into the timeline that will run `attempt_infection()` again.

```rust
{{#include ../../models/disease_model/src/transmission_manager.rs:12:41}}
```

Read through this implementation and make sure you understand how it accomplishes the three tasks above. A few observations:

- The `#[allow(clippy::cast_precision_loss)]` is optional; without it the compiler will warn you about converting `population` 's integral type `usize` to the floating point type `f64`, but we know that this conversion is safe to do in this context.
- If the sampled person is not susceptible, then the only thing this function does is schedule the next attempt at infection.
- The time at which the next attempt is scheduled is sampled randomly from the exponential distribution according to our abstract model and using the random number source `TransmissionRng` that we defined specifically for this purpose.
- None of this code refers to the people module (except to import the types `InfectionStatus` and `InfectionStatusValue`) or the infection manager we are about to write.

> [!INFO] Random Number Generators
> Each module generally defines its own random number source with `define_rng!`, avoiding interfering with the random number sources used elsewhere in the simulation in order to preserve determinism. In Monte Carlo simulations, *deterministic* pseudorandom number sequences are desirable because they ensure reproducibility, improve efficiency, provide control over randomness, enable consistent statistical testing, and reduce the likelihood of bias or error. These qualities are critical in scientific computing, optimization problems, and simulations that require precise and *verifiable* results.
