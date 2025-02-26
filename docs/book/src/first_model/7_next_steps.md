# Next Steps
We have created several new modules. We need to make sure they are each initialized with the `Context` before the simulation starts. Below is `main.rs` in its entirety.
```rust
// main.rs
{{#include ../../models/disease_model/src/main.rs}}
```

Exercises:

1. Currently the simulation runs until `MAX_TIME` has passed even if every single person has been infected and has recovered. Add a check somewhere that calls `context.shutdown()` if there is no more work for the simulation to do. Where should this check live?
2. Analyze the data output by the incident reporter. Plot the number of people with each `InfectionStatus` on the same axis to see how they change over the course of the simulation. Are the curves what we expect to see given our abstract model?
3. Add another person property that moderates the risk of infection of the individual. (Imagine, for example, people wearing face masks for an airborne illness.) Give a randomly sampled subpopulation that intervention, and add a check to the transmission module to see if the person that we are attempting to infect has that property. Change the probability of infection accordingly.
