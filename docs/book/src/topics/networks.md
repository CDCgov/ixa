# Networks

The `network` module stores directed edges between entities of one type, split
into _layers_. A layer is a kind of relationship (household, school, workplace)
and the same two people can be connected in several layers at once.

## Defining a network

```rust
define_entity!(Person);

// One layer per setting; each edge carries an f64 (here, a relative rate).
define_network!(Contact, Person, layers = { Household, School }, data = f64);

// A single layer with no edge data.
define_network!(InfectionTree, Person);
```

`layers` makes an enum whose variants are the layers. Without it the network is
a unit struct with one layer. `data` is the type stored on every edge and
defaults to `()`.

## Adding and removing edges

Every method takes the layer first; the entity and data types follow from it.

```rust
context.add_edge(InfectionTree, infector, infectee, ())?;
context.add_edge_bidi(Contact::Household, p1, p2, 1.0)?; // both directions
context.add_edges(Contact::School, pairs.iter().map(|&(a, b)| (a, b, 0.5)))?;

let removed = context.remove_edge(Contact::School, p1, p2); // Option<Edge>
context.clear_layer(Contact::School);         // close the schools
```

Two rules are enforced: no edge from an entity to itself, and at most one edge
per (source, neighbor) in a layer. Both return an `IxaError`.
`add_edge_bidi` checks both directions before inserting either. Edge data is
stored as given, so validate rates when you load them.

## Reading edges

Reads borrow from the store, so iterating allocates nothing.

```rust
for &layer in Contact::ALL {
    for edge in context.get_edges(layer, p1) {
        // edge.neighbor: PersonId, edge.data: f64
    }
}

context.get_edge(Contact::Household, p1, p2); // Option<&Edge>
context.get_degree(Contact::Household, p1);
```

Edges come back in insertion order. `remove_edge` swaps the last edge into the
gap, so order is only stable while no edge is removed.

## Sampling a contact

```rust
define_rng!(TransmissionRng);

let contact = context.sample_edge(TransmissionRng, Contact::Household, infector);
let contact = context.sample_edge_weighted(
    TransmissionRng,
    Contact::School,
    infector,
    |edge| edge.data,
);
```

Both return `Option<&Edge>`, `None` when the person has no edges in that
layer. The weighted form draws in proportion to the closure's value without
allocating; weights must be finite and non-negative with at least one positive.

## Example

`examples/network-hhmodel` builds a `Contact` network with `Household`,
`AgeUnder5` and `Age5to17` layers from CSV files and samples contacts from every
layer each time step.
