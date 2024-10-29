use ixa::context::{Context, IxaEvent};
use ixa::define_data_plugin;
use ixa_derive::IxaEvent;
use std::collections::HashMap;

use serde::{Deserialize, Serialize};

#[derive(Debug, Hash, Eq, PartialEq, Clone, Copy, Serialize, Deserialize)]
pub enum InfectionStatus {
    S,
    I,
    R,
}

#[derive(Copy, Clone, IxaEvent)]
pub struct InfectionStatusEvent {
    #[allow(dead_code)]
    pub prev_status: InfectionStatus,
    pub updated_status: InfectionStatus,
    pub person_id: usize,
}

pub trait ContextPeopleExt {
    fn create_person(&mut self);
    fn get_person_status(&self, person_id: usize) -> InfectionStatus;
    fn set_person_status(&mut self, person_id: usize, infection_status: InfectionStatus);
    fn get_population(&self) -> usize;
}

struct PeopleData {
    people_map: HashMap<usize, InfectionStatus>,
}
// Register the data container in context
define_data_plugin!(
    PeoplePlugin,
    PeopleData,
    PeopleData {
        people_map: HashMap::<usize, InfectionStatus>::new()
    }
);

impl ContextPeopleExt for Context {
    fn create_person(&mut self) {
        let people_data_container = self.get_data_container_mut(PeoplePlugin);
        let person_id = people_data_container.people_map.len();
        people_data_container
            .people_map
            .insert(person_id, InfectionStatus::S);
    }

    fn get_person_status(&self, person_id: usize) -> InfectionStatus {
        let people_data_container = self.get_data_container(PeoplePlugin);
        let person_status: InfectionStatus = *people_data_container
            .unwrap()
            .people_map
            .get(&person_id)
            .expect("Person does not exist");
        person_status
    }

    fn set_person_status(&mut self, person_id: usize, infection_status: InfectionStatus) {
        let previous_status: InfectionStatus = self.get_person_status(person_id);
        let people_data_container = self.get_data_container_mut(PeoplePlugin);
        let inf_status = people_data_container
            .people_map
            .get_mut(&person_id)
            .unwrap();

        *inf_status = infection_status;

        self.emit_event(InfectionStatusEvent {
            prev_status: previous_status,
            person_id,
            updated_status: infection_status,
        });
    }

    fn get_population(&self) -> usize {
        let people_data_container = self.get_data_container(PeoplePlugin);
        let population: usize = people_data_container.unwrap().people_map.len();
        population
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use ixa::context::Context;
    use ixa::define_data_plugin;
    use std::collections::HashMap;

    #[test]
    fn test_person_creation() {
        let mut context = Context::new();
        context.create_person();
        let person_id = 0;
        let person_status: InfectionStatus = context.get_person_status(person_id);
        assert_eq!(person_status, InfectionStatus::S);

        context.set_person_status(person_id, InfectionStatus::I);
        assert_eq!(context.get_person_status(person_id), InfectionStatus::I);
    }

    #[test]
    fn test_infection_status_event() {
        let mut context = Context::new();
        context.create_person();
        let person_id = 0;

        context.subscribe_to_event::<InfectionStatusEvent>(move |_, event| {
            let test_infection_status = event.updated_status;
            let test_prev_infection_status = event.prev_status;
            assert_eq!(test_infection_status, InfectionStatus::I);
            assert_eq!(test_prev_infection_status, InfectionStatus::S);
        });

        context.set_person_status(person_id, InfectionStatus::I);
        context.execute();
    }

    #[test]
    fn test_get_population() {
        let mut context = Context::new();
        let pop_size = 10;
        for _ in 0..pop_size {
            context.create_person();
        }
        let pop_size_context = context.get_population();
        assert_eq!(pop_size_context, pop_size);
    }
}
