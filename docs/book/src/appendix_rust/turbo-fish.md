# Specifying Generic Types and the Turbo Fish

Rust and ixa in particular make heavy use of type generics, a way of writing a
single piece of code, a function for example, for a whole family of types at
once. A nice feature of Rust is that the compiler can often infer the generic
types at the point the function is used instead of relying on the programmer to
specify the types explicitly.

## Examples

The compiler's ability to infer the types of generic functions means that for
most of the common functions the types do not need to be specified with turbo
fish notation:

```rust
// The compiler "magically" knows to use the `get_property` method that fetches
// `InfectionStatus` because of the type of the variable being assigned to.
let status: InfectionStatus = context.get_property(person_id);
// Explicit types are almost never required for `Context::set_property`, because
// the types of the entity ID and property value are almost always already known.
context.set_property(other_person_id, status);
```

The generic types for querying and sampling
methods can usually be inferred by the compiler:

```rust
// A silly example, but no turbo fish is required.
context.with_query_results(
    with!(Person, Age(30), Alive(true)),
    |people_set| println("{:?}", people_set)
);
```

A few methods always require the user to specify the generic type when they are
called:

```rust
// Which entity are we counting? Here the return type is always `usize`.
let population: usize = context.get_entity_count::<Person>();
// Specify which property of which entity you'd like to index.
context.index_property::<Person, Age>();
// Specify the report to add.
context.add_report::<IncidenceReportItem>("incidence")?;
// Specify the event to subscribe to.
context.subscribe_to_event::<InfectionStatusEvent>(handle_infection_status_change);
```

In the last example above, the concrete type we specify is actually a type
alias:

```rust
pub type InfectionStatusEvent = PropertyChangeEvent<Person, InfectionStatus>;
```

While it is not strictly necessary to define this type alias, you can see that
the notation gets rather gnarly without it:

```rust

context.subscribe_to_event::<PropertyChangeEvent<Person, InfectionStatus>>(handle_infection_status_change);
```
