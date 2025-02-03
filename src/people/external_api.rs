use crate::people::PeoplePlugin;
use crate::Context;
use crate::IxaError;
use crate::PersonId;

pub(crate) trait ContextPeopleExtCrate {
    fn get_person_property_by_name(
        &self,
        name: &str,
        person_id: PersonId,
    ) -> Result<String, IxaError>;
}

impl ContextPeopleExtCrate for Context {
    fn get_person_property_by_name(
        &self,
        name: &str,
        person_id: PersonId,
    ) -> Result<String, IxaError> {
        let data_container = self.get_data_container(PeoplePlugin).unwrap();
        let type_id = *data_container
            .people_types
            .borrow()
            .get(name)
            .ok_or(IxaError::IxaError(format!("No property '{name}'")))?;

        let index = data_container.get_index_ref(type_id).unwrap(); // This should exist
        Ok((index.get_display)(self, person_id))
    }
}

#[cfg(test)]
mod test {
    use super::ContextPeopleExtCrate;
    use crate::people::{define_person_property, ContextPeopleExt};
    use crate::Context;
    use crate::ContextRandomExt;

    define_person_property!(Age, u8);

    #[test]
    fn get_property_string() {
        let mut context = Context::new();
        context.init_random(42);

        let person1 = context.add_person((Age, 10)).unwrap();
        let age = context.get_person_property_by_name("Age", person1).unwrap();
        assert_eq!(age, "10");
    }

    #[test]
    fn get_unknown_property_string() {
        let mut context = Context::new();
        context.init_random(42);

        let person1 = context.add_person((Age, 10)).unwrap();
        let age = context.get_person_property_by_name("Unknown", person1);
        assert!(age.is_err());
    }
}
