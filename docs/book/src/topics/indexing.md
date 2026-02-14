# Understanding Indexing in Ixa

## Syntax and Best Practices

Syntax:

```rust
// For single property indexes
// Somewhere during the initialization of `context`:
context.index_property::<Person, Age>();

// For multi-indexes
// Where properties are defined:
define_multi_property!((Name, Age, Weight), Person);
// Somewhere during the initialization of `context`:
context.index_property::<Person, (Name, Age, Weight)>();
```

Best practices:

- Index a property to improve performance of queries of that property.
- Create a multi-property index to improve performance of queries involving
  multiple properties.
- The cost of creating indexes is increased memory use, which can be significant
  for large populations. So it is best to only create indexes / multi-indexes
  that actually improve model performance.
- It may be best to call `context.index_property::\<Entity, Property<Entity>>()`
  in the `init()` method of the module in which the property is defined, or you
  can put all of your `Context::index_property` calls together in a main
  initialization function if you prefer.
- It is not an error to call `Context::index_property` in the middle of a
  running simulation or to call it twice for the same property.
- Calling `Context::index_property` enables indexing and catches the index up to
  the current population at the time of the call.

## Property Value Storage in Ixa

To understand why some operations in Ixa are slow without an index, we need to
understand how property data is stored internally and how an index provides Ixa
an alternative view into that data.

In Ixa, each agent in a simulation—such as a person in a disease transmission
model—is associated with a unique row of data. This data is stored in columnar
form, meaning each property or field of a person (such as infection status, age,
or household) is stored as its own column. This structure allows for fast and
memory-efficient processing.

Let’s consider a simple example with two fields: `PersonId` and
`InfectionStatus`.

- `PersonId`: a unique identifier for each individual, which is represented as
  an integer internally (e.g., 1001, 1002, 1003, …).
- `InfectionStatus`: a status value indicating whether the individual is
  `susceptible`, `infected`, or `recovered`.

At a particular time during our simulation, we might have the following data:

| `PersonId` | `InfectionStatus` |
| :--------- | :---------------- |
| 0          | susceptible       |
| 1          | infected          |
| 2          | susceptible       |
| 3          | recovered         |
| 4          | susceptible       |
| 5          | susceptible       |
| 6          | infected          |
| 7          | susceptible       |
| 8          | infected          |
| 9          | susceptible       |
| 10         | recovered         |
| 11         | infected          |
| 12         | infected          |
| 13         | infected          |
| 14         | recovered         |

In the default representation used by Ixa, each field is stored as a column.
Internally, however, `PersonId` is _not_ stored explicitly as data. Instead, it
is implicitly defined by the _row number_ in the columnar data structure. That
is:

- The row number acts as the unique index (`PersonId`) for each individual.
- The `InfectionStatus` values are stored in a single contiguous array, where
  the entry at position `i` gives the status for the person with `PersonId`
  equal to `i`.

In this default layout, accessing the infection status for a person is a simple
array lookup, which is _extremely fast_ and requires minimal memory overhead.

But suppose instead of looking up the infection status of a particular
`PersonId`, you wanted to look up which `PersonId`'s were associated to a
particular infection status, say, `infected`. If the the property is not
indexed, Ixa has to scan through the entire column and collect all `PersonId`'s
(row numbers) for which `InfectionStatus` has the value `infected`, and it has
to do this each and every time we run a query for that property. If we do this
frequently, all of this scanning can add up to quite a long time!

## Property Index Structure

We could save a lot of time if we scanned through the `InfectionStatus` column
once, collected the `PersonId` 's for each `InfectionStatus` value, and just
reused this table each time we needed to do this lookup. That's all an index is!

The index for our example column of data:

| `InfectionStatus` | List of `PersonId` 's    |
| ----------------- | ------------------------ |
| `susceptible`     | `\[0, 2, 4, 5, 7, 9]`    |
| `infected`        | `\[1, 6, 8, 11, 12, 13]` |
| `recovered`       | `\[3, 10, 14]`           |

An index in Ixa is just a map between a property _value_ and the list of all
`PersonId`'s having that value. Now looking up the `PersonId`'s for a given
property value is (almost) as fast as looking up the property value for a given
`PersonId`.

## The Costs of Creating an Index

There are two costs you have to pay for indexing:

1. The index needs to be maintained as the simulation evolves the state of the
   population. Every change to any person's infection status needs to be
   reflected in the index. While this operation is fast for a single update, it
   isn't instant, and the accumulation of millions of little updates to the
   index can add up to a real runtime cost.
2. The index uses memory. In fact, it uses more memory than the original column
   of data, because it has to store _both_ the `InfectionStatus` values (in our
   example) _and_ the `PersonId` values, while the original column only stores
   the `InfectionStatus` (the `PersonId`'s were implicitly the row numbers).

> [!INFO] Creating vs. Maintaining an Index
>
> Suspiciously missing from this list of costs is the initial cost of scanning
> through the property column to create the index in the first place, but
> actually whether you maintain the index from the very beginning or you index
> it all at once doesn't matter: the sum of all the small efforts to update the
> index every time a person is added is equal to the cost of creating the index
> from scratch for an existing set of data.

Usually scanning through the whole property column is so slow relative to
maintaining an index that the extra computational cost of maintaining the index
is completely dwarfed by the time savings, even for infrequently queried
properties. In other words, in terms of running time, an index is almost always
worth it. For smaller population sizes in particular, at worst you shouldn't see
a meaningful slow-down.

Memory use is a different story. In a model with tens of millions of people and
many properties, you might want to be more thoughtful about which properties you
index, as memory use can reach into the gigabytes. While we are in an era where
tens of gigabytes of RAM is commonplace in workstations, cloud computing costs
and the selection of appropriate virtual machine sizes for experiments in
production recommend that we have a feel for whether we really need the
resources we are using.

> [!TIP] A query might be the wrong tool for the job
>
> Sometimes, the best way to address a slow query in your model isn’t to add
> indexes, but to remove the query entirely. A common scenario is when you want
> to report on some aggregate statistics, for example, the total number of
> people having each infectiousness status. It might be much better to just
> track the aggregate value directly than to run a query for it every time you
> want to write it to a report. As usual, when it comes to performance issues,
> measure your specific use case to know for sure what the best strategy is.

## Multi Property Indexes

To speed up queries involving multiple properties, use a _multi-property index_
(or _multi-index_ for short), which indexes multiple properties _jointly_.
Suppose we have the properties `AgeGroup` and `InfectionStatus`, and we want to
speed up queries of these two properties:

```rust
let query = with!(Person, AgeGroup(30), InfectionStatus::Susceptible);
let age_and_status = context.query_result_iterator(query); // Bottleneck
```

We could index `AgeGroup` and `InfectionStatus` individually, but in this case
we can do even better with a multi-index, which treats the pairs of values
`(AgeGroup, InfectionStatus)` as if it were a single value. Such a multi-index
might look like this:

| `(AgeGroup, InfectionStatus)` | `PersonId`'s                     |
| :---------------------------- | :------------------------------- |
| `(10, susceptible)`           | `\[16, 27, 31]`                  |
| `(10, infected)`              | `\[38]`                          |
| `(10, recovered)`             | `\[18, 23, 29, 34, 39]`          |
| `(20, susceptible)`           | `\[12, 25, 26]`                  |
| `(20, infected)`              | `\[2, 3, 9, 14, 17, 19, 28, 33]` |
| `(20, recovered)`             | `\[13, 20, 22, 30, 37]`          |
| `(30, susceptible)`           | `\[0, 1, 11, 21]`                |
| `(30, infected)`              | `\[5, 6, 7, 10, 15, 24, 32]`     |
| `(30, recovered)`             | `\[4, 8, 35, 36]`                |

Ixa hides the boilerplate required for creating a multi-index with the macro
`define_multi_property!`:

```rust
define_multi_property!((AgeGroup, InfectionStatus), Person);
```

Creating a multi-index _does not_ automatically create indexes for each of the
properties individually, but you can do so yourself if you wish, for example, if
you had other single property queries you want to speed up.

## The Benefits of Indexing - A Case Study

In the Ixa source repository you will find the `births-deaths` example in the
`examples/` directory. You can build and run this example with the following
command:

```bash
cargo run --example births-death
```

Now let's edit the `input.json` file and change the population size to 1000:

```json
{
    "population": 1000,
    "max_time": 780.0,
    "seed": 123,
    ⋮
}
```

We can time how long the simulation takes to run with the `time` command. Here's
what the command and output look like on my machine:

```bash
$ time cargo run --example births-deaths
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.30s
     Running `target/debug/examples/births-deaths`
cargo run --example births-deaths  362.55s user 1.69s system 99% cpu 6:06.35 total
```

For a population size of only 1000 it takes more than six minutes to run!

Let's index the `InfectionStatus` property. In
`examples/births-deaths/src/lib.rs` we add the following line somewhere in the
`initialize()` function:

```rust
context.index_property::<Person, InfectionStatus>();
```

We also need to import `InfectionStatus` by putting
`use crate::population_manager::InfectionStatus;` near the top of the file. To
be fair, let's compile the example separately so we don't include the compile
time in the run time:

```bash
cargo build --example births-deaths
```

Now run it again:

```bash
$ time cargo run --example births-deaths
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.09s
     Running `target/debug/examples/births-deaths`
cargo run --example births-deaths  5.79s user 0.07s system 97% cpu 5.990 total
```

From six minutes to six seconds! This kind of dramatic speedup is typical with
indexes. It allows models that would otherwise struggle with a population size
of 1000 to handle populations in the tens of millions.

Exercises:

1. Even six seconds is an eternity for modern computer processors. Try to get
   this example to run with a population of 1000 in ~1 second\*, _two orders of
   magnitude_ faster than the unindexed version, by indexing other additional
   properties.
2. Using only a single property index of `InfectionStatus` and a single
   multi-index, get this example to run in ~0.5 seconds. This illustrates that
   it's better to index the _right_ properties than to just index everything.

\*Your timings will be different but should be roughly proportional to these.
