/// Synthesized per-type methods that encapsulate the person
/// property type.
use crate::people::HashValueType;
use crate::{Context, ContextPeopleExt, PersonId, PersonProperty};

type PersonCallback<T> = dyn Fn(&Context, PersonId) -> T;

pub(crate) struct Methods {
    // A callback that calculates the 128-bit hash of a person's current property value
    pub(super) indexer: Box<PersonCallback<HashValueType>>,

    // A callback that calculates the display value of a person's current property value
    pub(super) get_display: Box<PersonCallback<String>>,
}

impl Methods {
    pub(super) fn new<T: PersonProperty>() -> Self {
        Self {
            indexer: Box::new(move |context: &Context, person_id: PersonId| {
                let value = context.get_person_property(person_id, T::get_instance());
                T::hash_property_value(&value)
            }),
            get_display: Box::new(move |context: &Context, person_id: PersonId| {
                let value = context.get_person_property(person_id, T::get_instance());
                T::get_display(&value)
            }),
        }
    }
}
