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

// An undirected network with two layers.
define_network!(Partnership, Person, layers = { Main, Casual }, undirected);
```

`layers` makes an enum whose variants are the layers. Without it the network is
a unit struct with one layer. `data` is the type stored on every edge and
defaults to `()`. `undirected` makes the network undirected (the default is
directed); `data`, `validate`, and `undirected` may appear in any order after
`layers`.

On an undirected network, `add_edge`, `add_edges`, `remove_edge`, `update_edge`,
and `add_or_update_edge` always act on both directions, `get_edge_count` and
`LayerView::edge_count` count connected pairs, and `LayerView::iter` yields each
pair once (from the lower id to the higher one).

A pair is one edge whichever order it is given in. Adding `(b, a)` when `(a, b)`
is already there is an `EdgeAlreadyExists` error, and `add_edges` adds nothing
if its input lists a pair in both orders. For an edge list that has every pair
twice, keep one order:

```rust
context.add_edges(Partnership::Main, rows.filter(|&(a, b, _)| a < b))?;
```

For input that may repeat a pair in either order, call `add_or_update_edge` for
each row instead: it sets the pair's data whether or not the pair exists, and
the last value given wins. Like `add_edge`, each call looks through the
entity's existing edges, so it is slow for entities with thousands of edges.

Edge data is checked when it is stored. By default `f64` and `f32` data must be
finite and non-negative, since it is usually a rate or weight, and any other
type is accepted as given. `validate` replaces the default with your own rule:

```rust
// Rates relative to a household contact, so nothing above 1 makes sense.
define_network!(
    Contact,
    Person,
    layers = { Household, School },
    data = f64,
    validate = |rate: &f64| (0.0..=1.0).contains(rate)
);
```

## Adding and removing edges

Every method takes the layer first; the entity and data types follow from it.

```rust
context.add_edge(InfectionTree, infector, infectee, ())?;
context.add_edge_bidi(Contact::Household, p1, p2, 1.0)?; // both directions
context.add_edges(Contact::School, pairs.iter().map(|&(a, b)| (a, b, 0.5)))?;
context.add_edges_bidi(Contact::School, pairs.iter().map(|&(a, b)| (a, b, 0.5)))?;

let removed = context.remove_edge(Contact::School, p1, p2); // Option<Edge>
let pair = context.remove_edge_bidi(Contact::Household, p1, p2); // Option<(Edge, Edge)>
context.clear_layer(Contact::School);         // close the schools

// Change an edge's data in place; Ok(false) if there is no such edge.
let updated = context.update_edge(Contact::School, p1, p2, |rate| *rate *= 0.5)?;

// Set an edge's data, adding the edge if there is none; returns the previous data.
let previous = context.add_or_update_edge(Contact::School, p1, p2, 0.25)?; // Option<f64>

// The same for both directions of a pair.
let updated = context.update_edge_bidi(Contact::School, p1, p2, |rate| *rate *= 0.5)?;
let previous = context.add_or_update_edge_bidi(Contact::School, p1, p2, 0.25)?; // (p1 -> p2, p2 -> p1)
```

Three rules are enforced: no edge from an entity to itself, at most one edge
per (source, neighbor) in a layer, and the data passes the network's
validation. Each returns an `IxaError`, and a failed call changes nothing: a
rejected update leaves the edge's previous data in place. `add_edge_bidi`
checks both directions before inserting either, and `remove_edge_bidi` removes
both or neither. `update_edge_bidi` runs its closure once, on the data of the
first direction, and stores the result on both edges; it changes nothing unless
both directions exist. `add_or_update_edge_bidi` adds whichever direction is
missing. `add_edges` and `add_edges_bidi` add every edge they are given or none.

Adding an edge looks through the source's existing edges for one to the same
neighbor, so building up an entity with thousands of edges one call at a time
gets slower with every edge. `add_edges` and `add_edges_bidi` check each entity
once for the whole batch: use them to load a network or to connect a large
group.

`update_edge` and `add_or_update_edge` keep an existing edge where it is.
Removing an edge and adding it back moves it, which changes which edge a given
seed samples.

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
context.get_edge_count(Contact::Household);      // edges in the layer, each direction counted
context.degree_distribution(Contact::Household); // counts[d] = people with degree d
```

Edges come back in insertion order. `remove_edge` swaps the last edge into the
gap, so order is only stable while no edge is removed.

### A whole layer

`get_layer` returns a view of one layer. It has the layer's edge count, an
iterator over every edge with its source, and the same per-entity reads as the
context. The view finds the layer once, so a loop over many people is faster
through it than through the context.

```rust
let partnerships = context.get_layer(Contact::Household);

partnerships.edge_count();    // same as context.get_edge_count(..)
partnerships.get_degree(p1);
partnerships.get_edges(p1);   // &[Edge]

// Every edge in the layer, by source id.
for (source, edge) in partnerships.iter() {
    // source: PersonId, edge.neighbor: PersonId, edge.data: f64
}
```

An edge added with `add_edge_bidi` is stored once in each direction, so it
appears twice in `iter` and counts twice in `edge_count`. `iter_bidi` gives one
direction of each pair: the edges from a lower id to a higher one (entity ids
are ordered, so `a < b` works in a model's own code too).

```rust
let ended: Vec<(PersonId, PersonId)> = context
    .get_layer(Contact::Household)
    .iter_bidi()
    .filter(|(_, edge)| edge.data < threshold)
    .map(|(source, edge)| (source, edge.neighbor))
    .collect();
for (a, b) in ended {
    let _ = context.remove_edge_bidi(Contact::Household, a, b);
}
```

The view borrows the context, so collect what to change before changing it.

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
allocating; weights must be finite and non-negative. A weight of 0 excludes a
neighbor, and if every weight is 0 the result is `None`, with no random number
drawn.

## Hierarchy: counties and households

A network has one entity type, so a hierarchy of places is built from two
pieces: a network at each level, and a property that points from the finer level
to the coarser one. Here people live in counties. Households are edges between
people, travel is an edge between counties, and `Home` links the two.

```rust
define_entity!(County);
define_entity!(Person);

// Which county a person lives in. Indexed, so "everyone in county c" is one lookup.
define_property!(struct Home(CountyId), Person);

// Coarse level: counties that exchange travellers. Edge data is the share of a
// resident's contacts that happen in the neighbouring county.
define_network!(Travel, County, data = f64);

// Fine level: who lives with whom.
define_network!(Household, Person);

define_rng!(ContactRng);
```

Build the county level first, then the people with their `Home`, then the
household edges:

```rust
context.index_property::<Person, Home>();

let a = context.add_entity(with!(County))?;
let b = context.add_entity(with!(County))?;
context.add_edge_bidi(Travel, a, b, 0.2)?;

let p1 = context.add_entity(with!(Person, Home(a)))?;
let p2 = context.add_entity(with!(Person, Home(a)))?;
context.add_edge_bidi(Household, p1, p2, ())?;
```

Drawing a contact walks down the hierarchy: a household edge, or else a county
(home, or a neighbour weighted by its travel share) and then a person who lives
there. The county step is a query on the indexed `Home` property, not an edge,
which is what keeps a county of thousands from being stored as a clique.

```rust
const HOUSEHOLD_SHARE: f64 = 0.5;

fn sample_contact(context: &Context, infector: PersonId) -> Option<PersonId> {
    if context.sample_bool(ContactRng, HOUSEHOLD_SHARE) {
        if let Some(edge) = context.sample_edge(ContactRng, Household, infector) {
            return Some(edge.neighbor);
        }
    }

    // Otherwise mix within a county: travel along an edge with probability equal
    // to the total outgoing share, else stay home.
    let Home(home) = context.get_property(infector);
    let away: f64 = context
        .get_edges(Travel, home)
        .iter()
        .map(|edge| edge.data)
        .sum();
    let county = if context.sample_bool(ContactRng, away) {
        context
            .sample_edge_weighted(ContactRng, Travel, home, |edge| edge.data)?
            .neighbor
    } else {
        home
    };

    // None when the draw lands on the infector: no contact this time.
    context
        .sample_entity(ContactRng, with!(Person, Home(county)))
        .filter(|&contact| contact != infector)
}
```

The same shape extends to more levels (tracts inside counties inside states):
one entity type and network per level that has its own relationships, and one
property per link between levels. Dense groups such as a whole county or school
stay as indexed properties; edges are for the sparse relationships, such as
households and which counties border which. `tests/network_hierarchy.rs` runs
this example and checks the contact proportions.

## Example

`examples/network-hhmodel` builds a `Contact` network with `Household`,
`AgeUnder5` and `Age5to17` layers from CSV files and samples contacts from every
layer each time step.
