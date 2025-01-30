use crate::{Context, ContextPeopleExt, IxaEvent, PersonId, PersonProperty};
use ixa_derive::IxaEvent;

/// Emitted when a new person is created
/// These should not be emitted outside this module
#[derive(Clone, Copy, IxaEvent)]
#[allow(clippy::manual_non_exhaustive)]
pub struct PersonCreatedEvent {
    /// The [`PersonId`] of the new person.
    pub person_id: PersonId,
}

/// Emitted when a person property is updated
/// These should not be emitted outside this module
#[derive(Copy, Clone)]
#[allow(clippy::manual_non_exhaustive)]
pub struct PersonPropertyChangeEvent<T: PersonProperty> {
    /// The [`PersonId`] that changed
    pub person_id: PersonId,
    /// The new value
    pub current: T::Value,
    /// The old value
    pub previous: T::Value,
}

impl<T: PersonProperty + 'static> IxaEvent for PersonPropertyChangeEvent<T> {
    fn on_subscribe(context: &mut Context) {
        if T::is_derived() {
            context.register_property::<T>();
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        define_derived_property, define_global_property, define_person_property,
        define_person_property_with_default, Context, ContextPeopleExt, PersonCreatedEvent,
        PersonId, PersonPropertyChangeEvent,
    };
    use std::cell::RefCell;
    use std::rc::Rc;

    define_person_property!(Age, u8);
    #[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
    pub enum AgeGroupValue {
        Child,
        Adult,
    }
    define_global_property!(Threshold, u8);
    define_derived_property!(IsEligible, bool, [Age], [Threshold], |age, threshold| {
        age >= threshold
    });

    define_derived_property!(AgeGroup, AgeGroupValue, [Age], |age| {
        if age < 18 {
            AgeGroupValue::Child
        } else {
            AgeGroupValue::Adult
        }
    });

    #[derive(Copy, Clone, PartialEq, Eq, Debug, Hash)]
    pub enum RiskCategoryValue {
        High,
        Low,
    }
    define_person_property!(RiskCategory, RiskCategoryValue);
    define_person_property_with_default!(IsRunner, bool, false);
    define_person_property!(RunningShoes, u8, |context: &Context, person: PersonId| {
        let is_runner = context.get_person_property(person, IsRunner);
        if is_runner {
            4
        } else {
            0
        }
    });

    #[test]
    fn observe_person_addition() {
        let mut context = Context::new();

        let flag = Rc::new(RefCell::new(false));
        let flag_clone = flag.clone();
        context.subscribe_to_event(move |_context, event: PersonCreatedEvent| {
            *flag_clone.borrow_mut() = true;
            assert_eq!(event.person_id.0, 0);
        });

        let _ = context.add_person(()).unwrap();
        context.execute();
        assert!(*flag.borrow());
    }

    #[test]
    fn observe_person_property_change() {
        let mut context = Context::new();

        let flag = Rc::new(RefCell::new(false));
        let flag_clone = flag.clone();
        context.subscribe_to_event(
            move |_context, event: PersonPropertyChangeEvent<RiskCategory>| {
                *flag_clone.borrow_mut() = true;
                assert_eq!(event.person_id.0, 0, "Person id is correct");
                assert_eq!(
                    event.previous,
                    RiskCategoryValue::Low,
                    "Previous value is correct"
                );
                assert_eq!(
                    event.current,
                    RiskCategoryValue::High,
                    "Current value is correct"
                );
            },
        );
        let person_id = context
            .add_person((RiskCategory, RiskCategoryValue::Low))
            .unwrap();
        context.set_person_property(person_id, RiskCategory, RiskCategoryValue::High);
        context.execute();
        assert!(*flag.borrow());
    }

    #[test]
    fn observe_person_property_change_with_set() {
        let mut context = Context::new();

        let flag = Rc::new(RefCell::new(false));
        let flag_clone = flag.clone();
        context.subscribe_to_event(
            move |_context, _event: PersonPropertyChangeEvent<RunningShoes>| {
                *flag_clone.borrow_mut() = true;
            },
        );
        let person_id = context.add_person(()).unwrap();
        // Initializer called as a side effect of set, so event fires.
        context.set_person_property(person_id, RunningShoes, 42);
        context.execute();
        assert!(*flag.borrow());
    }

    #[test]
    fn get_person_property_change_event() {
        let mut context = Context::new();
        let person = context.add_person((Age, 17)).unwrap();

        let flag = Rc::new(RefCell::new(false));

        let flag_clone = flag.clone();
        context.subscribe_to_event(
            move |_context, event: PersonPropertyChangeEvent<AgeGroup>| {
                assert_eq!(event.person_id.0, 0);
                assert_eq!(event.previous, AgeGroupValue::Child);
                assert_eq!(event.current, AgeGroupValue::Adult);
                *flag_clone.borrow_mut() = true;
            },
        );
        context.set_person_property(person, Age, 18);
        context.execute();
        assert!(*flag.borrow());
    }
}
