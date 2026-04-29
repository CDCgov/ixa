# Example: Agent-based SEIR model with contact networks

This example demonstrates a network model in `ixa`.

There are three data files:

- `Households.csv` represents 500 households of size 1-12. Individuals have age
  category and sex properties. Within the model, these individuals are placed in
  a densely connected network.
- `AgeUnder5Edges.csv` contains the edges connecting those aged under 5.
- `Age5to17Edges.csv` contains the edges connecting those aged 5-17.

The simulation runs via:

- `parameters.rs` sets up global properties for the model.
- `loader.rs` reads in the `Household.csv` file and instantiates the people in it.
- `network.rs` forms a dense network of household contacts, then reads in the other
  contact files and instantiates those network edges. The edges are tracked as
  model entities. This module also selects which individuals have effective contact
  during each time period.
- `seir.rs` manages transmission, infections, and disease trajectories.
- `incidence_report.rs` sets up a report with information on who became infected
  by whom during the simulation and saves the information to a csv in an `\output`
  folder.

Note that the relative rate of transmission between households (relative to within households) is a property of the network edges. For technical reasons, ixa properties must implement `Eq`, which Rust floats do not. This example manually implements equality logic; future ixa versions may have other solutions.

## How to run the model

`cargo run --example network-hhmodel`

To run the model with logging turned on:

`cargo run --example network-hhmodel -- --log-level info`

To see all details of the plans being added:

`cargo run --example network-hhmodel -- --log-level trace`
