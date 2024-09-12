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

define_person_property!(Age, u8);
define_person_property!(RiskCategoryType, RiskCategory);

pub fn init(context: &mut Context) {
    // Load csv and deserialize records
    let mut reader = csv::Reader::from_path("./examples/load-people/people.csv").unwrap();

    for result in reader.deserialize() {
        let record: PeopleRecord = result.expect("Failed to parse record");

        context.before_person_added(move |context, person_id| {
            context.set_person_property(person_id, Age, record.age);
            context.set_person_property(person_id, RiskCategoryType, record.risk_category);
        });

        let _person = context.add_person();
        // Setting person properties at this point actually happens *after*
        // any initialization callbacks, but before regular event callbacks.
        // Kind of weird I guess
    }
}
