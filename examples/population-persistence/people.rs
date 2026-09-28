use ixa::prelude::*;

define_entity!(Person);

define_property!(struct Age(u8), Person);

define_property!(
    enum InfectionStatus {
        Susceptible,
        Infected,
        Recovered,
    },
    Person,
    default_const = InfectionStatus::Susceptible
);

define_property!(
    struct Vaccinated(bool),
    Person,
    default_const = Vaccinated(false)
);
