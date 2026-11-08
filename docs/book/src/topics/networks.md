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

let removed = context.remove_edge(Contact::School, p1, p2); // Option<Edge>
let pair = context.remove_edge_bidi(Contact::Household, p1, p2); // Option<(Edge, Edge)>
context.clear_layer(Contact::School);         // close the schools

// Change an edge's data in place; Ok(false) if there is no such edge.
let updated = context.update_edge(Contact::School, p1, p2, |rate| *rate *= 0.5)?;
```

Three rules are enforced: no edge from an entity to itself, at most one edge
per (source, neighbor) in a layer, and the data passes the network's
validation. Each returns an `IxaError`, and a failed call changes nothing: a
rejected update leaves the edge's previous data in place. `add_edge_bidi`
checks both directions before inserting either, and `remove_edge_bidi` removes
both or neither.

`update_edge` keeps the edge where it is. Removing an edge and adding it back
moves it, which changes which edge a given seed samples.

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
context.degree_distribution(Contact::Household); // counts[d] = people with degree d
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
