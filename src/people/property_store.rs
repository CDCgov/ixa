//! A `PropertyStore<P: PersonProperty>` stores the values of the property `P` for each person.
//!
//! The values are stored in a vector, in which the value at index `i` of the vector
//! is an `Option<P::Value>`, the value for the property for the person with ID
//! `PersonId(i)`. We store an option to allow for a property value to be unset. As a
//! space optimization, the vector is only large enough to hold the values for the people
//! that have values set. All values beyond the vector's length are interpreted as unset.
//! A property can have a default initial value, either a constant or a function of the
//! `Context` and `PersonId`. in the case of a function, the value is inserted into the vector
//! at the first time it is accessed, possibly enlarging the vector.
//!
//! Some operations, like querying and sampling, require the values to be initialized for the entire
//! population, because an immutable reference is held during the operation, and so initial values
//! cannot be set at that time. This is done by calling `initialize_uninitialized_properties` on the
//! `PropertyStore<P>` instance. (This mechanism is in analogy to `Index::index_unindexed_people`.)

use crate::people::property::PropertyInitializationKind;
use crate::{Context, ContextPeopleExt, PersonId, PersonProperty};
use std::any::Any;

pub type BxPropertyStore = Box<dyn TypeErasedPropertyStore>;

// A Person is represented by a `PersonId`, which is a wrapper for a number between 0 and
// `POPULATION_SIZE - 1`. The values of a property are stored in a vector, in which the value at
// index `i` of the vector is the value for the property for the person with ID `PersonId(i)`.
pub(in crate::people) struct PropertyStore<P: PersonProperty> {
    pub(in crate::people) values: Vec<Option<P::Value>>,
    #[allow(dead_code)]
    initialized_up_to: usize,
}

impl<P: PersonProperty> PropertyStore<P> {
    pub(crate) fn new() -> Self {
        PropertyStore {
            values: Vec::default(),
            initialized_up_to: 0,
        }
    }
}

pub trait TypeErasedPropertyStore: Any {
    /// Used for debugging, delegates to `PersonProperty::name()`
    #[allow(dead_code)]
    #[must_use]
    fn name(&self) -> &'static str;
    /// Delegates to `PersonProperty::is_required()`
    #[must_use]
    fn is_required(&self) -> bool;
    #[allow(dead_code)]
    fn initialize_uninitialized_properties(&mut self, context: &Context);
}

impl<P: PersonProperty> TypeErasedPropertyStore for PropertyStore<P> {
    fn name(&self) -> &'static str {
        P::name()
    }

    fn is_required(&self) -> bool {
        P::is_required()
    }

    fn initialize_uninitialized_properties(&mut self, context: &Context) {
        if P::property_initialization_kind() != PropertyInitializationKind::Dynamic {
            return;
        }
        let current_population = context.get_current_population();

        // Make room for at the additional values that will be inserted.
        self.values
            .reserve(current_population - self.initialized_up_to);

        // It is possible that the vector is longer than `self.initialized_up_to` if values
        // have been set for sufficiently large `PersonId`s. We still have to scan through
        // all slots beyond `self.initialized_up_to` to initialize any missing values.
        let current_length = self.values.len();
        for i in self.initialized_up_to..current_population {
            if i < current_length && self.values[i].is_none() {
                self.values[i] = Some(P::compute(context, PersonId(i)));
            } else if i >= current_length {
                self.values.push(Some(P::compute(context, PersonId(i))));
            }
        }
    }
}
