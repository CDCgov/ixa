/// Synthesized per-type methods that encapsulate the person
/// property type.
use crate::people::index::IndexValue;
use crate::{ContextPeopleExt, PersonId, PersonProperty};

type PersonCallback<T, C> = dyn Fn(&C, PersonId) -> T;

pub(crate) struct Methods<C: ContextPeopleExt> {
    // A callback that calculates the IndexValue of a person's current property value
    pub(super) indexer: Box<PersonCallback<IndexValue, C>>,

    // A callback that calculates the display value of a person's current property value
    pub(super) get_display: Box<PersonCallback<String, C>>,
}

impl<C: ContextPeopleExt> Methods<C> {
    pub(super) fn new<T: PersonProperty>() -> Self {
        Self {
            indexer: Box::new(move |context: &C, person_id: PersonId| {
                let value = context.get_person_property(person_id, T::get_instance());
                IndexValue::compute(&value)
            }),
            get_display: Box::new(move |context: &C, person_id: PersonId| {
                let value = context.get_person_property(person_id, T::get_instance());
                T::get_display(&value)
            }),
        }
    }
}
