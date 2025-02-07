# Example: Agent-based SEIR model with contact networks

This example demonstrates the use of the `network` module of `ixa`.

There are three CSV files:
* `Households.csv` represents 500 households of size 1-12.  Individuals have age category and sex properties.  Within the model, these individuals are placed in a densely connected network.
* `AgeUnder5Edges.csv` contains the edges connecting those aged under 5
* `Age5to17Edges.csv` contains the edges connecting those aged 5-17.

In `network.rs`, three corresponding edge types are created using the
`define_edge_type!` macro and the networks are formed by adding edges  to the context using `add_edge_bidi`.

In `seir.rs`, a SEIR model is implemented with different betas by network edge type.  Edge queries (`get_matching_edges`) allow us to identify the neighbors of the infected individuals and consider whether they become exposed.

`loader.rs` reads in the `Household.csv` file and `parameters.rs` sets up global properties for the SEIR model.

`incidence_report.rs` sets up a report with information on who became infected by whom during the simulation and saves the information to a csv in an `\output` folder.
