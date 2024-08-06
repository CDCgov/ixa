use serde_derive::Serialize;
use strum_macros::EnumIter;

#[derive(Debug, Hash, Eq, PartialEq, Serialize, EnumIter)]
pub enum DiseaseStatus {
    S,
    E,
    I,
    R,
    D,
}

#[derive(Debug, Eq, PartialEq, Hash, EnumIter)]
pub enum HealthStatus {
    Asymp,
    Symp,
    Hospitalized,
    Recovered,
}
eosim::define_person_property_from_enum!(DiseaseStatus, DiseaseStatus::S);
eosim::define_person_property_from_enum!(HealthStatus, HealthStatus::Asymp);
