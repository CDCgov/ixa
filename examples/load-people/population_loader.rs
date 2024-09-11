use ixa::context::Context;
use ixa::define_person_property;
use ixa::people::ContextPeopleExt;
use serde::Deserialize;

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

define_person_property!(Age, u8, 0);
define_person_property!(RiskCategoryType, RiskCategory, RiskCategory::Low);

pub fn init(context: &mut Context) {
    // Load csv and deserialize records
    let mut reader = csv::Reader::from_path("./examples/load-people/people.csv").unwrap();

    for result in reader.deserialize() {
        let record: PeopleRecord = result.expect("Failed to parse record");
        let _person = context
            .create_person()
            .set_person_property(Age, record.age)
            .set_person_property(RiskCategoryType, record.risk_category)
            .insert();
    }
}
