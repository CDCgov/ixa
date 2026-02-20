# Next Steps

We have created several new modules. We need to make sure they are each
initialized with the `Context` before the simulation starts. Below is `main.rs`
in its entirety.

```rust
// main.rs
{{#rustdoc_include ../../models/disease_model/src/main.rs}}
```

## Exercises

1. Currently the simulation runs until `MAX_TIME` even if every single person
   has been infected and has recovered. Add a check somewhere that calls
   `context.shutdown()` if there is no more work for the simulation to do. Where
   should this check live? _Hint: Use `context.query_entity_count`._
2. Analyze the data output by the incident reporter. Plot the number of people
   with each `InfectionStatus` on the same axis to see how they change over the
   course of the simulation. Are the curves what we expect to see given our
   abstract model? _Hint: Remember this model has a fixed force of infection,
   unlike a typical SIR model._
3. Add another property that moderates the risk of infection of the individual.
   (Imagine, for example, that some people wash their hands more frequently.)
   Give a randomly sampled subpopulation that intervention and add a check to
   the transmission module to see if the person that we are attempting to infect
   has that property. Change the probability of infection accordingly.
   _Hint: You will probably need some new constants, a new person property, a new
   random number generator, and the `Bernoulli` distribution._
