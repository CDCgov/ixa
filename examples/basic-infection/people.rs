use ixa::context::Context;
use std::collections::HashMap;

#[derive(Debug, Hash, Eq, PartialEq)]
pub enum InfectionStatus {
    S,
    I,
    R,
}

pub trait PeopleContext {
    fn create_new_person(&self, person_id: u64);
}

impl PeopleContext for Context {
    fn create_new_person(&self, person_id: u64) {
        let mut population = HashMap::<u64, InfectionStatus>::new();
        population.insert(person_id, InfectionStatus::S);
        println!("Person created {:?}", population);
    }
}
