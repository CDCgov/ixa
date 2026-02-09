# Specifying Generic Types and the Turbo Fish

Rust and ixa in particular make heavy use of type generics, a way of writing a
single piece of code, a function for example, for a whole family of types at
once. A nice feature of Rust is that the compiler can often infer the generic
types at the point the function is used instead of relying on the programmer to
specify the types explicitly.

## An example with `Context::add_entity()`

Suppose we want to initialize a population:

```rust
define_entity!(Person);
define_property!(
    // The type of the property
    enum InfectionStatus {S,I,R},
    // The entity the property is associated with
    Person,
    // The property's default value for newly created `Person` entities
    default_const = InfectionStatus::S
);

/// Populates the "world" with people.
pub fn init(context: &mut Context) {
    for _ in 0..1000 {
        context.add_entity((InfectionStatus::S, )).expect("failed to add person");
    }
}
```

During the initialization of our population, we explicitly told ixa to create a
new _susceptible_ person, that is, with the `InfectionStatus::S` property value.
However, when we defined the `InfectionStatus` property with the
`define_property!` macro, we specified a default initial value with
`default_const = InfectionStatus::S`. Since `InfectionStatus` has a default
value, we don't need to supply a value for it when calling
`context.add_entity(...)`. But remember, the compiler infers _which_ entity to
create based on the property values we supply, and if we don't supply _any_
property values, we need another way to specify the entity to create.

The `Context::add_entity` function is actually a whole family of functions
`Context::add_entity\<E: Entity, PL: PropertyList>`, one function for each
concrete `Entity` type `E` and `PropertyList` type `PL`. When we call
`context.add_entity(...)` in our code with a tuple of properties (the
initialization list), the Rust compiler looks at the initialization list and
uses it to infer the concrete types `E` and `PL`. But when the initialization
list is `PL=()` (the empty list), the compiler doesn't know what the `Entity`
type `E` should be. The Rust language allows us to tell it explicitly using the
"turbo fish" notation:

```rust
context.add_entity::<Person, ()>(()).expect("failed to add person");
```

Actually, the compiler _always_ already knows the `PropertyList` type `PL`, so
we can use the "wildcard" `_` (underscore) to tell the compiler to infer that
type itself:

```rust
context.add_entity::<Person, _>(()).expect("failed to add person");
```

There is another way to give the compiler enough information to infer the
`Entity` type, namely by specifying the return type we are expecting. In our
case, we just throw the returned `PersonId` away, but suppose we want to refer
to the newly created person. We could write:

```rust
let person_id: PersonId = context.add_entity(()).expect("failed to add person");
```

The `Entity` type `E` must be `Person`, because that is the only way
`Context::add_entity\<E: Entity, PL: PropertyList>` can return a `PersonId` (a
type alias for `EntityId\<Person>`). You can use this trick even if you never
use the returned `PersonId`, but in such a case it's best practice to signal
this intent by using the special "don't care" variable `_` (underscore):

```rust
let _: PersonId = context.add_entity(()).expect("failed to add person");
```

You do not have to learn the rules for when specifying the types using turbo
fish notation is required. The compiler will let you know. For `add_entity`,
always specifying the returned type means you'll never have to worry about turbo
fish notation.

## Preferred Idiom for `Context::sample_entity()`

The `Context::sample_entity()` method especially deserves discussion, because we
often want to immediately use the returned value. If we try to use the standard
Rust idiom to express this, we have to specify the types using turbo fish, which
is awkward and ugly:

```rust
// Sample from the entire population by supplying the "empty" query. The last two `_`s are for the query type and
// RNG type, both of which the compiler can infer.
if let Some(person_id) = context.sample_entity::<Person, _, _>(TransmissionRng, ()) {
    // Do something with `person_id`...
}
```

Since we are sampling from the entire population, if `sample_entity` returns
`None`, then the population is empty, and we clearly have a bug in our code, in
which case the best thing to do is to crash the program and fix the bug. Thus,
instead of the `if let Some(...) =` construct, it's actually better to just call
`unwrap` on the returned value in this case. Here is a much more readable and
simple way to write the code:

```rust
// Sample from the entire population by supplying the "empty" query. The compiler infers which entity to sample
// from the type of the variable we assign to.
let person_id: PersonId = context.sample_entity(()).unwrap();
// Do something with `person_id`...
```

If you really want to check for the `None` case in your code, assign the return
value to a variable of type `Option\<PersonId>` instead of immediately
unwrapping the `PersonId` value. Then you can use `if let Some(...) =` or a
`match` statement at your preference:

```rust
let maybe_person_id: Option<PersonId> = context.sample_entity(());
match maybe_person_id {
    Some(person_id) => {
        // Do something with `person_id`
    }
    None => {
        // Handle the empty population case
    }
}
```

## Other Examples

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

As with `Context::add_entity`, the generic types for querying and sampling
methods can usually be inferred by the compiler except when the "empty" query is
provided:

```rust
// A silly example, but no turbo fish is required.
context.with_query_results(
    (Age(30), Alive(true)),
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

