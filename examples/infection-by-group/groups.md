This example defines the `ixa` `group` component. Groups are different from person properties, though both "assign" some characteristic to people. This document explains the characteristics of groups and the API that `ixa` exposes to users to enable them to use groups.

## Overview

Let us describe the use case for a group and define a group so that it's difference from person properties is clear. First, a group is a collection of people. For it to be useful to define a collection of people, they must all have some shared attribute, but that shared attribute does not necessarily need to be defined as a person property. This paradigm is useful for three reasons: (a) it is straightforward to get all people who have the shared property that defines their group, (b) people can be part of multiple groups and it is straightforward to get all their group ownerships, and (c) groups can have other information (for instances, available resources) attached to them (though this is really just an external hash map).

## Use Cases

From a modeling perspective, groups enable an abstraction beyond explicitly needing to model each individual's interactions.
1) Imagine people who visit the library, the supermarket, and/or the DMV after the work day is over. Rather than modeling the interactions an individual may have in each of these settings explicitly, place them in the library group, the supermarket group, and the DMV group. This is an example of a group type where the person can be zero to many of these different types of "community" groups. Each of these groups may fit into a broader type of group of, say, "government" settings (library, DMV) or "private" settings (supermarket).
2) Imagine children who are part of a school. Children are always part of at least one classroom, but they may be part of multiple classrooms (say, certain children from certain classrooms combine partially through the day to all go to gym class together). Instead of explicitly modeling the interactions of each child in each set of classrooms, children are assigned to at least one classroom group. Multiple classrooms make up a school, and schools are part of districts, so there is a hierarchy in student placements.
3) Imagine a population of people in a city, half of whom live in shelters and have of whom live in the broader population. There are different shelters, but they all fit into the broade hierarchy of "shelters". People can only ever be part of either a shelter or the broader population, so this type of group enforces that people are part of one and only one group.

These use cases explain key properties of a group

## Properties
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
