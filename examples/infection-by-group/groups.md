## Overview
Groups have the following properties:

- They have a unique group identifier, such as `Region` or `Classroom`
- A membership mapping, which can be one to one, one to many, or zero to many
- A number of people in the group

A person can go in an out of groups, but the membership mapping must still be satisfied.

Model authors should be able to do the following:
* Create a group and define its membership mapping
* Define parent-child relationships between groups
* Assign people to group
* Change someone's group membership, which includes adding them or removing to groups
* Given a group, get all the members of a group at a given time
* Given a group get its parent
* Given a group, all of its child groups
* Given a person, list all their groups stratified by group id
* Attach resources to groups

## Examples
- Regions (1-1): Every person is in exactly 1 state.
- Classroom (1-many): Every person is in one or more classrooms
- TransportationModes (0-many): People can use multiple transportation modes, or not move at all

## Architecture

There are two possible structures we could consider here; one is a model
where groups are special entities which must be assigned members, the other is
that a group is an association with some set of matching person property values.

### Explicit group members

Each person has an association with a group via assignment:

```rust
enum Regions {
  Califonria,
  NewYork,
  //...
}
define_group!(
  RegionId,
  GroupMappings::one_to_one,
)

person.assign_to_group(RegionId, Regions.California);
```


### Group members as queries of person properties

Groups define a query, which represents a set of matching person property values

```rust
enum Regions {
  California,
  NewYork,
  //...
}
define_group!(
  RegionGroupId,
  GroupMappings::one_to_one,
  (RegionId)
)

define_person_property!(
  RegionId,
  Regions
)

// This causes the group to update because it matches the query
person.set_person_property(RegionId, Regions.California);
```
