use ixa::prelude::*;

// A household that contains people
define_entity!(struct Household { HouseholdTag });

// The identifier defined by the synthetic population
define_property!(HouseholdTag, u32);
