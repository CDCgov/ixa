/// Creates a query matching all entities of a given type, optionally filtered by properties.
///
/// # Examples
///
/// ```ignore
/// // Add an entity with default properties
/// let query = with!(Person);
/// context.add_entity(query)?;
///
/// // An inline query matching a single property
/// let person = context.sample_entity(MyRng, with!(Person, Age(12)))?;
///
/// // A query matching multiple properties
/// let query = with!(Person, Age(12), RiskCategory::High);
/// let count = context.count_entities(query);
/// ```
#[macro_export]
macro_rules! with {
    // No properties - generates empty tuple query
    ($entity:ty) => {
        $crate::EntityPropertyTuple::<$entity, _>::new(())
    };
    // One or more properties
    ($entity:ty, $($prop:expr),+ $(,)?) => {
        $crate::EntityPropertyTuple::<$entity, _>::new(($($prop,)+))
    };
}

#[cfg(test)]
mod tests {
    use crate::context::Context;
    use crate::entity::ContextEntitiesExt;
    use crate::random::ContextRandomExt;
    use crate::{define_entity, define_property, define_rng, impl_property};

    define_entity!(TestPerson);
    define_property!(struct Age(u8), TestPerson, default_const = Age(0));
    define_property!(
        enum Risk {
            High,
            Low,
        },
        TestPerson
    );
    define_rng!(AllMacroTestRng);

    #[test]
    fn all_macro_with_add_entity() {
        let mut context = Context::new();

        // Use with! macro to add an entity
        let person = context
            .add_entity(with!(TestPerson, Age(42), Risk::High))
            .unwrap();

        // Verify properties were set correctly
        assert_eq!(context.get_property::<TestPerson, Age>(person), Age(42));
        assert_eq!(context.get_property::<TestPerson, Risk>(person), Risk::High);
    }

    #[test]
    fn all_macro_with_sample_entity() {
        let mut context = Context::new();
        context.init_random(42);

        // Add some entities
        let p1 = context
            .add_entity(with!(TestPerson, Age(30), Risk::High))
            .unwrap();
        let _ = context
            .add_entity(with!(TestPerson, Age(30), Risk::Low))
            .unwrap();
        let _ = context
            .add_entity(with!(TestPerson, Age(25), Risk::High))
            .unwrap();

        // Sample from entities matching the query
        let sampled =
            context.sample_entity(AllMacroTestRng, with!(TestPerson, Age(30), Risk::High));
        assert_eq!(sampled, Some(p1));
    }

    #[test]
    fn all_macro_with_sample_entity_no_match() {
        let mut context = Context::new();
        context.init_random(42);

        // Add some entities that don't match the query
        let _ = context
            .add_entity(with!(TestPerson, Age(30), Risk::Low))
            .unwrap();

        // Sample should return None when no entities match
        let sampled =
            context.sample_entity(AllMacroTestRng, with!(TestPerson, Age(30), Risk::High));
        assert_eq!(sampled, None);
    }

    // Demonstrate that `with!` can disambiguate entities in otherwise ambiguous cases.
    use crate::entity::EntityId;
    define_entity!(TestMammal);
    define_entity!(TestAvian);
    define_property!(struct IsBipedal(bool), TestMammal, default_const = IsBipedal(false));
    impl_property!(IsBipedal, TestAvian, default_const = IsBipedal(true));

    #[test]
    fn all_macro_disambiguates_entities() {
        let mut context = Context::new();

        context
            .add_entity(with!(TestAvian, IsBipedal(true)))
            .unwrap();
        context
            .add_entity(with!(TestMammal, IsBipedal(true)))
            .unwrap();

        let result = context.query_result_iterator(with!(TestMammal, IsBipedal(true)));
        assert_eq!(result.count(), 1);

        let sampled_id = context
            .sample_entity(AllMacroTestRng, with!(TestAvian, IsBipedal(true)))
            .unwrap();
        let expected_id: EntityId<TestAvian> = EntityId::new(0);
        assert_eq!(sampled_id, expected_id);
    }
}
