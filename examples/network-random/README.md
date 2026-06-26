# Example: Random connection network

Minimal example of a random network model. At the start of the simulation, in `network.rs`, the random graph is built with [`rust-igraph`](https://totoro-jam.github.io/rust-igraph/) and then translted into instantiated ixa entities, `Person` and `Edge`. In `infection.rs`, upon infection, that infectee's connections are scheduled for the next generation of onward infection. For simplicity, there is a single generation interval used for all infector-infectee pairs, essentially producing discrete generations.
