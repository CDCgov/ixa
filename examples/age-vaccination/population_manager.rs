use std::path::Path;

use ixa::context::Context;
use ixa::{define_person_property, define_rng};
use ixa::people::{ContextPeopleExt, PersonId};
use ixa::global_properties::ContextGlobalPropertiesExt;
use ixa::random::ContextRandomExt;
use serde::Deserialize;
use rand_distr::Exp;

use crate::Parameters;

define_rng!(PeopleRng);

#[derive(Deserialize, Copy, Clone, PartialEq, Eq, Debug)]
pub enum RiskCategory {
    High,
    Low,
}

#[derive(Deserialize, Debug)]
struct PeopleRecord {
    age: u8,
    risk_category: RiskCategory,
}

define_person_property!(Age, u8);
define_person_property!(RiskCategoryType, RiskCategory);

fn create_person_from_record(context: &mut Context, record: &PeopleRecord) -> PersonId {
    let person = context.add_person();
    context.initialize_person_property(person, Age, record.age);
    context.initialize_person_property(person, RiskCategoryType, record.risk_category);

    person
}

fn schedule_birth(context: &mut Context) {
    let parameters = context.get_global_property_value(Parameters).clone();
    let person = context.add_person();
    context.initialize_person_property(person, Age, 0);

    // What to do with people who are just born? is risk category a derived property?
    context.initialize_person_property(person, RiskCategoryType, RiskCategory::Low);

    let next_birth_event = context.get_current_time() +
        context.sample_distr(PeopleRng, Exp::new(parameters.birth_rate).unwrap());
    println!("Next birth event: {:?}", next_birth_event);
    context.add_plan(next_birth_event,
        move |context| {
            schedule_birth(context);
    });
}

fn schedule_death(context: &mut Context) {
    let parameters = context.get_global_property_value(Parameters).clone();
    let id = context.sample_range(PeopleRng, 0..context.get_current_population());
    let person = context.get_person_id(id);
    context.remove_person(person);
    let next_death_event = context.get_current_time() +
        context.sample_distr(PeopleRng, Exp::new(parameters.death_rate).unwrap());

    println!("Attempting to remove {:?} - Next death event: {:?}", person, next_death_event);
    
    context.add_plan(next_death_event,
        move |context| {
            schedule_death(context);
    });
}

pub fn init(context: &mut Context) {
    // Load csv and deserialize records
    let parameters = context.get_global_property_value(Parameters).clone();
    
    let current_dir = Path::new(file!()).parent().unwrap();
    let mut reader = csv::Reader::from_path(current_dir.join(parameters.people_file)).unwrap();

    for result in reader.deserialize() {
        let record: PeopleRecord = result.expect("Failed to parse record");
        create_person_from_record(context, &record);
    }

    // Plan for births and deaths
    context.add_plan(0.0, |context| {schedule_birth(context)});
    
    context.add_plan(0.0, |context| {schedule_death(context)});
}

