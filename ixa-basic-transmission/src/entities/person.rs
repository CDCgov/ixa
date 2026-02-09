use ixa::prelude::*;

// A person; the main entity in the model
define_entity!(struct Person {
    Age,
    IsAlive = true,
    InfectionStatus = InfectionStatus::Susceptible,
    HouseholdTag,
});

// Age defined by the synthetic population
define_property!(Age, u8);

// Whether the person is alive or not
define_property!(IsAlive, bool);

// The infection status of the person
define_property!(
    enum InfectionStatus {
        Susceptible,
        Infected,
        Recovered,
    }
);

// Reference to a household defined by the synthetic population
define_property!(HouseholdTag, u32);
