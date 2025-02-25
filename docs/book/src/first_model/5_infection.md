# The Infection Manager
 The infection manager is responsible for the evolution of an infected person after they have been infected. In this simple model, there is only thing for the infection manager to do: schedule the time an infected person recovers. We've already seen how to change a person's `InfectionStatus` property and how to schedule plans on the timeline in the transmission module. But how does the infection manager know about new infections?
## Events
Modules can subscribe to events. The infection manager registers a function with Ixa that will be called in response to a change in a particular property.
```rust
{{#include ../../models/disease_model/src/infection_manager.rs:16}}
```
This line isn't defining a new struct or even a new type. Rather, it defines an alias for `PersonPropertyChangeEvent\<T>` with the generic type  `T` instantiated with the property we want to monitor, `InfectionStatus`. This is effectively the name of the event we subscribe to in the module's `init()` function:
```rust
{{#include ../../models/disease_model/src/infection_manager.rs:48:51}}
```
The event handler is just a regular Rust function that takes a `Context` and an `InfectionStatusEvent`, the latter of which holds the `PersonId` of the person whose `InfectionStatus` changed, the current `InfectionStatusValue`, and the previous `InfectionStatusValue`.
```rust
{{#include ../../models/disease_model/src/infection_manager.rs:36:46}}
```
We only care about new infections in this model.

## Scheduling Recovery
As in `attempt_infection()`, we sample the recovery time from the exponential distribution with mean `INFECTION_DURATION`. We define a random number source for this module's exclusive use with `define_rng!(InfectionRng)`.
```rust
{{#include ../../models/disease_model/src/infection_manager.rs:20:34}}
```
Notice that this time the plan is again just a Rust function, but this time it takes the form of a closure rather than a traditionally define function. This is convenient when the function is only a line or two.

> [!INFO] Closures and Captured Variables
>  The `move` keyword in Rust closures instructs the closure to take ownership of any variables it uses from its surrounding context—these are known as captured variables. Normally, when a closure refers to variables defined outside of its own body, it borrows them, which means it uses references to those values. However, with `move`, the closure takes full ownership by moving the variables into its own scope. This is especially useful when the closure must outlive the current scope or be passed to another thread, as it ensures that the closure has its own independent copy of the data without relying on references that might become invalid.

