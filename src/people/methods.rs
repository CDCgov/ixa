/// Synthesized per-type methods that encapsulate the person
/// property type.
///
/// `PersonProperty` is not object safe, but we need to work with them generically. For most of
/// the code, we can use generics on functions, but because we need to store _values_ in indexes,
/// we need a pointer to a function from the particular `PersonProperty` type the value came from.
/// We could live without this if it weren't for the external API requiring access using only
/// the `TypeId`.
use crate::people::index::IndexValue;
use crate::{Context, ContextPeopleExt, PersonId, PersonProperty};

type PersonCallback<T> = dyn Fn(&Context, PersonId) -> T;

pub(crate) struct Methods {
    // A callback that calculates the IndexValue of a person's current property value
    pub(super) indexer: Box<PersonCallback<IndexValue>>,

    // A callback that calculates the display value of a person's current property value
    pub(super) get_display: Box<PersonCallback<String>>,
}

impl Methods {
    pub(super) fn new<T: PersonProperty>() -> Self {
        Self {
            indexer: Box::new(move |context: &Context, person_id: PersonId| {
                let value = context.get_person_property(person_id, T::get_instance());
                IndexValue::compute(&value)
            }),
            get_display: Box::new(move |context: &Context, person_id: PersonId| {
                let value = context.get_person_property(person_id, T::get_instance());
                format!("{value:?}")
            }),
        }
    }
}
