Many ixa simulations have functionality which depends on people that match
a certain set of properties. Some examples:

* We want to apply a policy intervention to  to all people with a given region
  and risk status;
* We want to select a random person to vaccinate in a given age group
* An individual's infectivity depends on on the total number of people in
  their immediate neighborhood

This requires that we can make *queries* against our population of a given set
of person properties and corresponding values.

```rust
let query = Query<Region, RiskStatus>(Regions::Arizona, RiskStatus::High);
for person in query.all() {
  context.apply_policy(person);
}
```

The naive way to implement this would be to iterate through each person in the
population, check their properties, and return anyone who matches. However,
in order to make this more efficient, we can take advantage of a data structure
for looking up a set of people *by value*: that is, an index.

## Indexes
Indexes allow for reverse lookup of the set of person ids that match a combination
of values. For example, an index for Region might look something like this:
```rust
{
  Regions::Arizona: [0, 4, 8]
  Regions::Alabama: [2, 5, 9]
  // Every value that is set in the simulation is listed here
}
```

Each unique matching combination of properties is called a Bucket. A query
corresponds to a particular Bucket in the index (for example,
people who are high risk in Arizona), but you may also want to iterate through
the total list of Buckets.

### Index creation and updating

When a new index is created, we have to instantiate it by iterating through every
person in the simulation and calling `get_person_property` on the relevant
properties. This may internally call initializers on person properties if necessary.
The key for the Bucket is computed as a hash of property values in some expected
order (so Arizona + High evaluates to the same key as High + Arizona).
Each person is added to the appropriate Bucket.

Once created, indexes must be kept up to date whenever a person is created
or their properties are set. Whenever this happens, some code internal to the
person module (i.e., called directly from `add_person` and `set_person_property`)
will will check the list of matching Buckets for each index – both
for previous values and current values – and update membership. It's important
for this to happen before any events are released into the system.

### API

Queries, which look up the corresponding bucket in an index:

```rust
let query = context.query::<RegionType, RiskStatusType>(Region::California, RiskStatus::High);
query.size();
let person = query.get_random_person();
```

Iterating through all buckets for an index:

```rust
let buckets = context.get_all_buckets::<Region, RiskStatus>();
for bucket in buckets {
  // ...
}

Indexes could be created manually; we could also create them automatically when you run a query.

```rust
context.create_index(RegionType);
context.create_index(RegionType, RiskStatusType);
```
