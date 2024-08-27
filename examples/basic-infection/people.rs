use ixa::context::Context;
use ixa::define_data_plugin;
use std::collections::HashMap;

#[derive(Debug, Hash, Eq, PartialEq, Clone, Copy)]
pub enum InfectionStatus {
    S,
    I,
    R,
}

pub trait PeopleContext {
    fn create_person(&mut self);
    fn get_person_status(&mut self, person_id:usize) -> InfectionStatus;
    fn set_person_status(&mut self, person_id:usize, infection_status:InfectionStatus);
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

// Add person
fn add_person<>(context: &mut Context) -> usize{
    // get data container
    let people_data_container = context
        .get_data_container_mut::<PeoplePlugin>();
    let person_id = people_data_container.people_map.keys().len();
    people_data_container.people_map.insert(person_id, InfectionStatus::S);
    return person_id;
}

// Get a person
fn get_person_status<>(context: &mut Context,  person_id: usize) ->  InfectionStatus {
    let people_data_container = context
        .get_data_container_mut::<PeoplePlugin>();
    return *people_data_container.people_map.get(&person_id).expect("Person does not exist");
}

// Modify person's status by Id

fn set_person_status<>(context: &mut Context, person_id: usize, infection_status: InfectionStatus) {
    let people_data_container = context
        .get_data_container_mut::<PeoplePlugin>();
    if let Some(inf_status) = people_data_container.people_map.remove(&person_id) {
        people_data_container.people_map.insert(person_id, infection_status); 
    }
}

impl PeopleContext for Context {
    fn create_person(&mut self) {
        //let  people_data_container = self.get_data_container_mut::<PeoplePlugin>();
        let person_id:usize = add_person(self);
        println!("Person created {:?}", person_id);
    }
    
    fn get_person_status(&mut self, person_id:usize) -> InfectionStatus{
        let person_status:InfectionStatus = get_person_status(self, person_id);
        return person_status;
    }

    fn set_person_status(&mut self, person_id:usize, infection_status:InfectionStatus){
        set_person_status(self, person_id, infection_status);
    }
    
}

#[cfg(test)]
mod test {
    use ixa::context::Context;
    use ixa::define_data_plugin;
    use std::collections::HashMap;
    use super::*;
    

    #[test]
    fn test_person_creation() {
        let mut context = Context::new();
        context.create_person();
        let person_id = 0;
        let person_status:InfectionStatus = context.get_person_status(person_id);
        assert_eq!(person_status, InfectionStatus::S);

        context.set_person_status(person_id, InfectionStatus::I);
        assert_eq!(context.get_person_status(person_id), InfectionStatus::I);
    }    
}
