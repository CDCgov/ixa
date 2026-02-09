//! Macros to correctly define and implement the `Entity` trait.

/// Defines a zero-sized struct with the right derived traits and implements the `Entity` trait.
///
/// # Forms
///
/// ## Simple entity (no associated properties)
/// ```rust,ignore
/// define_entity!(Person);
/// ```
///
/// ## Entity with associated properties (validated builder)
/// ```rust,ignore
/// define_entity!(struct Person {
///     Age,                                              // required (must be set)
///     Weight = 0.0,                                     // defaulted
///     Property<InfectionStatus> = InfectionStatus::S,   // defaulted enum
/// });
///
/// let person = context.add_entity(
///     Person::build()
///         .age(10_u8)
///         .build()?    // validates required fields
/// )?;
/// ```
///
/// The property-bearing form:
/// - Generates `impl PropertyDef<Person>` for each listed property
/// - Generates a `PersonBuilder` with setter methods and a `build()` method
/// - `build()` returns `Result<PersonInit, String>`, validating required fields
/// - `PersonInit` implements `PropertyList<Person>` for use with `add_entity`
#[macro_export]
macro_rules! define_entity {
    // Simple form (no properties)
    ($entity_name:ident) => {
        #[allow(unused)]
        #[derive(Default, Debug, PartialEq, Eq, Clone, Copy)]
        pub struct $entity_name;

        $crate::impl_entity!($entity_name);
    };

    // Form with property declarations
    (struct $entity_name:ident { $($prop_decl:tt)* }) => {
        // Define the entity struct and implement Entity trait
        $crate::define_entity!($entity_name);

        // Process property declarations with required/defaulted tracking
        $crate::define_entity!(@process_props $entity_name, required: [], defaulted: [], $($prop_decl)*);
    };

    // === Internal rules for parsing property declarations ===

    // Terminal: no more properties to parse. Generate builder.
    (@process_props $entity:ident,
        required: [$( ($r_prop:ty, $r_field:ident, $r_value_ty:ty) )*],
        defaulted: [$( ($d_prop:ty, $d_field:ident, $d_value_ty:ty, { $($d_default:tt)* }) )*],
    ) => {
        $crate::define_entity!(@generate_builder $entity,
            required: [$( ($r_prop, $r_field, $r_value_ty) )*],
            defaulted: [$( ($d_prop, $d_field, $d_value_ty, { $($d_default)* }) )*]
        );
    };

    // Parse a required property: `PropType,`
    (@process_props $entity:ident,
        required: [$($req:tt)*],
        defaulted: [$($def:tt)*],
        $prop:ident, $($rest:tt)*
    ) => {
        $crate::impl_property_for_entity!($prop, $entity);
        $crate::define_entity!(@process_props $entity,
            required: [$($req)* ($prop, $prop, <$prop as $crate::entity::property::IsProperty>::Value)],
            defaulted: [$($def)*],
            $($rest)*
        );
    };

    // Parse a required Property<T>: `Property<T>,`
    (@process_props $entity:ident,
        required: [$($req:tt)*],
        defaulted: [$($def:tt)*],
        Property<$inner:ident>, $($rest:tt)*
    ) => {
        $crate::define_entity!(@emit_property_for_entity $entity, $inner);
        $crate::define_entity!(@process_props $entity,
            required: [$($req)* ($crate::entity::property::Property<$inner>, $inner, $inner)],
            defaulted: [$($def)*],
            $($rest)*
        );
    };

    // Parse a defaulted property: `PropType = default_value,`
    (@process_props $entity:ident,
        required: [$($req:tt)*],
        defaulted: [$($def:tt)*],
        $prop:ident = $default:expr, $($rest:tt)*
    ) => {
        $crate::impl_property_for_entity!($prop, $entity, default_const = $default);
        $crate::define_entity!(@process_props $entity,
            required: [$($req)*],
            defaulted: [$($def)* ($prop, $prop, <$prop as $crate::entity::property::IsProperty>::Value, { $default })],
            $($rest)*
        );
    };

    // Parse a defaulted Property<T>: `Property<T> = default_value,`
    (@process_props $entity:ident,
        required: [$($req:tt)*],
        defaulted: [$($def:tt)*],
        Property<$inner:ident> = $default:expr, $($rest:tt)*
    ) => {
        $crate::define_entity!(@emit_property_for_entity $entity, $inner, default_const = $default);
        $crate::define_entity!(@process_props $entity,
            required: [$($req)*],
            defaulted: [$($def)* ($crate::entity::property::Property<$inner>, $inner, $inner, { $default })],
            $($rest)*
        );
    };

    // --- No trailing comma variants (last item in the list) ---

    // Required property without trailing comma
    (@process_props $entity:ident,
        required: [$($req:tt)*],
        defaulted: [$($def:tt)*],
        $prop:ident
    ) => {
        $crate::define_entity!(@process_props $entity, required: [$($req)*], defaulted: [$($def)*], $prop,);
    };

    // Required Property<T> without trailing comma
    (@process_props $entity:ident,
        required: [$($req:tt)*],
        defaulted: [$($def:tt)*],
        Property<$inner:ident>
    ) => {
        $crate::define_entity!(@process_props $entity, required: [$($req)*], defaulted: [$($def)*], Property<$inner>,);
    };

    // Defaulted property without trailing comma
    (@process_props $entity:ident,
        required: [$($req:tt)*],
        defaulted: [$($def:tt)*],
        $prop:ident = $default:expr
    ) => {
        $crate::define_entity!(@process_props $entity, required: [$($req)*], defaulted: [$($def)*], $prop = $default,);
    };

    // Defaulted Property<T> without trailing comma
    (@process_props $entity:ident,
        required: [$($req:tt)*],
        defaulted: [$($def:tt)*],
        Property<$inner:ident> = $default:expr
    ) => {
        $crate::define_entity!(@process_props $entity, required: [$($req)*], defaulted: [$($def)*], Property<$inner> = $default,);
    };

    // Helper: emit impl_property_for_entity for Property<T> (required)
    (@emit_property_for_entity $entity:ident, $inner:ident) => {
        $crate::impl_property_for_entity!(
            Property<$inner>,
            $entity
        );
    };

    // Helper: emit impl_property_for_entity for Property<T> (with default)
    (@emit_property_for_entity $entity:ident, $inner:ident, default_const = $default:expr) => {
        $crate::impl_property_for_entity!(
            Property<$inner>,
            $entity,
            default_const = $default
        );
    };

    // Generate validated builder struct, init struct, and PropertyList impl
    (@generate_builder $entity:ident,
        required: [$( ($r_prop:ty, $r_field:ident, $r_value_ty:ty) )*],
        defaulted: [$( ($d_prop:ty, $d_field:ident, $d_value_ty:ty, { $($d_default:tt)* }) )*]
    ) => {
        $crate::paste::paste! {
            /// Builder for creating entities with validated property values.
            ///
            /// Required properties are stored as `Option` and must be set before calling `build()`.
            /// Defaulted properties are pre-initialized with their default values.
            #[derive(Debug, Clone, Copy)]
            pub struct [<$entity Builder>] {
                $(
                    [<$r_field:snake>]: Option<$r_value_ty>,
                )*
                $(
                    [<$d_field:snake>]: $d_value_ty,
                )*
            }

            impl $entity {
                /// Create a new builder for this entity type.
                #[allow(unused)]
                pub fn new() -> [<$entity Builder>] {
                    [<$entity Builder>] {
                        $(
                            [<$r_field:snake>]: None,
                        )*
                        $(
                            [<$d_field:snake>]: { $($d_default)* },
                        )*
                    }
                }
            }

            impl [<$entity Builder>] {
                $(
                    #[allow(unused)]
                    pub fn [<$r_field:snake>](mut self, value: $r_value_ty) -> Self {
                        self.[<$r_field:snake>] = Some(value);
                        self
                    }
                )*
                $(
                    #[allow(unused)]
                    pub fn [<$d_field:snake>](mut self, value: $d_value_ty) -> Self {
                        self.[<$d_field:snake>] = value;
                        self
                    }
                )*

                /// Validate all required fields are set and produce a validated init struct.
                pub fn build(self) -> Result<[<$entity Init>], $crate::IxaError> {
                    Ok([<$entity Init>] {
                        $(
                            [<$r_field:snake>]: self.[<$r_field:snake>]
                                .ok_or_else(|| $crate::IxaError::from(format!(
                                    "required property {} not set",
                                    stringify!($r_field)
                                )))?,
                        )*
                        $(
                            [<$d_field:snake>]: self.[<$d_field:snake>],
                        )*
                    })
                }
            }

            /// Validated init struct for creating entities.
            ///
            /// Produced by the builder's `build()` method after validation.
            /// All fields are guaranteed to have values.
            #[derive(Debug, Clone, Copy)]
            pub struct [<$entity Init>] {
                $(
                    [<$r_field:snake>]: $r_value_ty,
                )*
                $(
                    [<$d_field:snake>]: $d_value_ty,
                )*
            }

            impl $crate::entity::property_list::PropertyList<$entity> for [<$entity Init>] {
                fn validate() -> Result<(), String> {
                    Ok(())
                }

                fn contains_properties(property_type_ids: &[std::any::TypeId]) -> bool {
                    let self_type_ids: &[std::any::TypeId] = &[
                        $(
                            <$r_prop as $crate::entity::property::PropertyDef<$entity>>::type_id(),
                        )*
                        $(
                            <$d_prop as $crate::entity::property::PropertyDef<$entity>>::type_id(),
                        )*
                    ];
                    property_type_ids.iter().all(|id| self_type_ids.contains(id))
                }

                fn contains_required_properties() -> bool {
                    true
                }

                fn set_values_for_entity(
                    &self,
                    entity_id: $crate::entity::EntityId<$entity>,
                    property_store: &$crate::entity::property_store::PropertyStore<$entity>,
                ) {
                    $(
                        {
                            let store = property_store.get::<$r_prop>();
                            store.set(entity_id, self.[<$r_field:snake>]);
                        }
                    )*
                    $(
                        {
                            let store = property_store.get::<$d_prop>();
                            store.set(entity_id, self.[<$d_field:snake>]);
                        }
                    )*
                }
            }
        }
    };
}

/// Implements the `Entity` trait for the given existing type and defines a type alias
/// of the form `MyEntityId = EntityId<MyEntity>`. For simple zero-sized types, use the
/// `define_entity!` macro instead, which will define the struct and derive all the super traits.
///
/// This macro ensures the correct implementation of the `Entity` trait. The tricky bit is the implementation of
/// `Entity::index`, which requires synchronization in multithreaded runtimes. This is an instance of
/// _correctness via macro_.
/// Implements the `Entity` trait for an existing struct, with optional property declarations.
///
/// # Forms
///
/// ## Simple form (no properties)
/// ```rust,ignore
/// impl_entity!(MyExistingStruct);
/// ```
///
/// ## With property declarations
/// ```rust,ignore
/// impl_entity!(struct MyExistingStruct {
///     Age,
///     Weight = 0.0,
///     Property<Status> = Status::Active,
/// });
/// ```
///
/// The property-bearing form does not define the struct — it must already exist.
/// It generates `PropertyDef` impls, a builder, and ctor registrations.
#[macro_export]
macro_rules! impl_entity {
    // Form with property declarations (struct already exists)
    (struct $entity_name:ident { $($prop_decl:tt)* }) => {
        $crate::impl_entity!($entity_name);
        $crate::define_entity!(@process_props $entity_name, required: [], defaulted: [], $($prop_decl)*);
    };

    // Simple form (no properties)
    ($entity_name:ident) => {
        // Alias of the form `MyEntityId = EntityId<MyEntity>`
        $crate::paste::paste! {
            #[allow(unused)]
            pub type [<$entity_name Id>] = $crate::entity::EntityId<$entity_name>;
        }

        impl $crate::entity::Entity for $entity_name {
            fn id() -> usize {
                // This static must be initialized with a compile-time constant expression.
                // We use `usize::MAX` as a sentinel to mean "uninitialized". This
                // static variable is shared among all instances of this concrete item type.
                static INDEX: std::sync::atomic::AtomicUsize =
                    std::sync::atomic::AtomicUsize::new(usize::MAX);

                // Fast path: already initialized.
                let index = INDEX.load(std::sync::atomic::Ordering::Relaxed);
                if index != usize::MAX {
                    return index;
                }

                // Slow path: initialize it.
                $crate::entity::entity_store::initialize_entity_index(&INDEX)
            }

            fn as_any(&self) -> &dyn std::any::Any {
                self
            }
            fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
                self
            }
        }

        // Using `ctor` to initialize entities at program start-up means we know how many entities
        // there are at the time any `EntityStore` is created, which means we never have
        // to mutate `EntityStore` to initialize an `Entity` that hasn't yet been accessed.
        // (The mutation happens inside of a `OnceCell`, which we can already have ready
        // when we construct `EntityStore`.) In other words, we could do away with `ctor`
        // if we were willing to have a mechanism for interior mutability for `EntityStore`.
        $crate::paste::paste! {
            $crate::ctor::declarative::ctor!{
                #[ctor]
                fn [<_register_entity_$entity_name:snake>]() {
                    $crate::entity::entity_store::add_to_entity_registry::<$entity_name>();
                }
            }
        }
    };
}

/// Declares that a parent entity is a group of child entities, establishing a
/// foreign key relationship via `ForeignEntityKey<Parent>` on the child.
///
/// This macro creates a `PropertyDef<Child>` implementation for `ForeignEntityKey<Parent>`.
///
/// # Example
///
/// ```rust,ignore
/// define_entity!(Household);
/// define_entity!(Person);
/// define_group!(Household of Person);
///
/// // Now you can use:
/// context.set_parent(Household, person_id, household_id);
/// let parent = context.get_parent(Household, person_id);
/// let children = context.get_children(Person, household_id);
/// ```
#[macro_export]
macro_rules! define_group {
    ($parent:ident of $child:ident) => {
        impl $crate::entity::property::PropertyDef<$child>
            for $crate::entity::foreign_entity_key::ForeignEntityKey<$parent>
        {
            type Value = Option<$crate::entity::EntityId<$parent>>;
            type CanonicalValue = Option<$crate::entity::EntityId<$parent>>;

            fn initialization_kind() -> $crate::entity::property::PropertyInitializationKind {
                $crate::entity::property::PropertyInitializationKind::Constant
            }

            fn compute_derived(
                _context: &$crate::Context,
                _entity_id: $crate::entity::EntityId<$child>,
            ) -> Self::Value {
                panic!("ForeignEntityKey is not a derived property")
            }

            fn default_const() -> Self::Value {
                None
            }

            fn make_canonical(value: Self::Value) -> Self::CanonicalValue {
                value
            }

            fn make_uncanonical(value: Self::CanonicalValue) -> Self::Value {
                value
            }

            fn get_display(value: &Self::Value) -> String {
                format!("{value:?}")
            }

            fn id() -> usize {
                static INDEX: std::sync::atomic::AtomicUsize =
                    std::sync::atomic::AtomicUsize::new(usize::MAX);

                let index = INDEX.load(std::sync::atomic::Ordering::Relaxed);
                if index != usize::MAX {
                    return index;
                }

                $crate::entity::property_store::initialize_property_id::<$child>(&INDEX)
            }

            fn index_id() -> usize {
                <Self as $crate::entity::property::PropertyDef<$child>>::id()
            }

            fn collect_non_derived_dependencies(_result: &mut $crate::HashSet<usize>) {}
        }

        $crate::paste::paste! {
            $crate::ctor::declarative::ctor! {
                #[ctor]
                fn [<_register_foreign_key_ $parent:snake _ $child:snake>]() {
                    $crate::entity::property_store::add_to_property_registry::<
                        $child,
                        $crate::entity::foreign_entity_key::ForeignEntityKey<$parent>,
                    >();
                }
            }
        }
    };
}

/// Alias for `define_entity!(struct ...)`. Prefer using `define_entity!` directly.
///
/// This macro delegates entirely to `define_entity!(struct $name { ... })`.
#[macro_export]
macro_rules! define_entity_with_properties {
    ($entity_name:ident { $($prop_decl:tt)* }) => {
        $crate::define_entity!(struct $entity_name { $($prop_decl)* });
    };
}

#[cfg(test)]
mod tests {
    #![allow(dead_code)]

    use crate::entity::property::Property;
    use crate::prelude::*;

    // New-style ZST property markers
    define_property!(ZstAge, u8);
    define_property!(ZstWeight, f64);
    define_property!(
        enum ZstStatus {
            S,
            I,
            R,
        }
    );

    // Entity with properties declared in define_entity!
    define_entity!(struct Animal {
        ZstAge,
        ZstWeight = 0.0,
        Property<ZstStatus> = ZstStatus::S,
    });

    #[test]
    fn test_new_style_builder_create() {
        let mut context = Context::new();

        // Create entity using builder
        let id = context
            .add_entity(
                Animal::new()
                    .zst_age(10_u8)
                    .zst_weight(55.5)
                    .build()
                    .unwrap(),
            )
            .unwrap();

        // Read back properties using turbofish
        let age: u8 = context.get_property::<_, ZstAge>(id);
        assert_eq!(age, 10);

        let weight: f64 = context.get_property::<_, ZstWeight>(id);
        assert_eq!(weight, 55.5);

        // ZstStatus should have its default since we didn't set it
        let status: ZstStatus = context.get_property::<_, Property<ZstStatus>>(id);
        assert_eq!(status, ZstStatus::S);
    }

    #[test]
    fn test_new_style_set_property() {
        let mut context = Context::new();

        let id = context
            .add_entity(Animal::new().zst_age(5_u8).build().unwrap())
            .unwrap();

        // Mutate property
        context.set_property::<_, ZstAge>(id, 20_u8);
        let age: u8 = context.get_property::<_, ZstAge>(id);
        assert_eq!(age, 20);

        // Mutate enum property
        context.set_property::<_, Property<ZstStatus>>(id, ZstStatus::I);
        let status: ZstStatus = context.get_property::<_, Property<ZstStatus>>(id);
        assert_eq!(status, ZstStatus::I);
    }

    #[test]
    fn test_new_style_defaults() {
        let mut context = Context::new();

        // Only set required property (ZstAge), rely on defaults for others
        let id = context
            .add_entity(Animal::new().zst_age(1_u8).build().unwrap())
            .unwrap();

        let weight: f64 = context.get_property::<_, ZstWeight>(id);
        assert_eq!(weight, 0.0);

        let status: ZstStatus = context.get_property::<_, Property<ZstStatus>>(id);
        assert_eq!(status, ZstStatus::S);
    }

    #[test]
    fn test_new_style_property_change_event() {
        use crate::entity::events::PropertyChangeEvent;

        type StatusChange = PropertyChangeEvent<Animal, Property<ZstStatus>>;

        let mut context = Context::new();

        define_data_plugin!(EventCount, usize, 0);

        context.subscribe_to_event::<StatusChange>(move |context, event| {
            assert_eq!(event.previous, ZstStatus::S);
            assert_eq!(event.current, ZstStatus::I);
            *context.get_data_mut(EventCount) += 1;
        });

        let id = context
            .add_entity(Animal::new().zst_age(3_u8).build().unwrap())
            .unwrap();
        context.set_property::<_, Property<ZstStatus>>(id, ZstStatus::I);
        context.execute();

        assert_eq!(*context.get_data(EventCount), 1);
    }

    #[test]
    fn test_marker_based_get_set() {
        let mut context = Context::new();

        let id = context
            .add_entity(Animal::new().zst_age(5_u8).build().unwrap())
            .unwrap();

        // get_property_value with marker
        let age: u8 = context.get_property_value(id, ZstAge);
        assert_eq!(age, 5);

        // set_property_value with marker
        context.set_property_value(id, ZstAge, 42_u8);
        assert_eq!(context.get_property_value(id, ZstAge), 42);

        // Enum property with Property<T> marker
        let status = context.get_property_value(id, Property::<ZstStatus>::new());
        assert_eq!(status, ZstStatus::S);

        context.set_property_value(id, Property::<ZstStatus>::new(), ZstStatus::R);
        assert_eq!(
            context.get_property_value(id, Property::<ZstStatus>::new()),
            ZstStatus::R
        );
    }

    #[test]
    fn test_add_entity_with_entity_marker() {
        // Simple entity without properties defined via define_entity!(struct ...)
        define_entity!(SimpleThing);

        let mut context = Context::new();

        // add_entity with entity marker directly (no properties)
        let id = context.add_entity(SimpleThing).unwrap();
        assert_eq!(context.get_entity_count::<SimpleThing>(), 1);

        let id2 = context.add_entity(SimpleThing).unwrap();
        assert_eq!(context.get_entity_count::<SimpleThing>(), 2);
        assert_ne!(id, id2);
    }

    #[test]
    fn test_impl_entity_with_properties() {
        // Define a struct manually
        #[derive(Default, Debug, PartialEq, Eq, Clone, Copy)]
        pub struct Vehicle;

        define_property!(Speed, f64);
        define_property!(
            enum Fuel {
                Gas,
                Electric,
                Hybrid,
            }
        );

        // Use impl_entity! to add Entity impl + property associations
        impl_entity!(struct Vehicle {
            Speed,
            Property<Fuel> = Fuel::Gas,
        });

        let mut context = Context::new();
        let car = context
            .add_entity(Vehicle::new().speed(60.0).build().unwrap())
            .unwrap();

        let speed: f64 = context.get_property_value(car, Speed);
        assert_eq!(speed, 60.0);

        let fuel = context.get_property_value(car, Property::<Fuel>::new());
        assert_eq!(fuel, Fuel::Gas);

        context.set_property_value(car, Property::<Fuel>::new(), Fuel::Electric);
        assert_eq!(
            context.get_property_value(car, Property::<Fuel>::new()),
            Fuel::Electric
        );
    }

    // === Tests for define_entity_with_properties! ===

    define_property!(CreatureAge, u8);
    define_property!(CreatureAlive, bool);
    define_property!(
        enum CreatureKind {
            Cat,
            Dog,
            Bird,
        }
    );

    define_entity_with_properties!(Creature {
        CreatureAge,
        CreatureAlive = true,
        Property<CreatureKind> = CreatureKind::Cat,
    });

    #[test]
    fn test_ewp_required_field_validation() {
        // build() without setting required field should error
        let result = Creature::new().build();
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("CreatureAge"));
    }

    #[test]
    fn test_ewp_default_values() {
        let mut context = Context::new();

        // Only set required field, defaults should apply
        let id = context
            .add_entity(Creature::new().creature_age(5_u8).build().unwrap())
            .unwrap();

        let alive: bool = context.get_property::<_, CreatureAlive>(id);
        assert!(alive); // default is true

        let kind: CreatureKind = context.get_property::<_, Property<CreatureKind>>(id);
        assert_eq!(kind, CreatureKind::Cat); // default is Cat
    }

    #[test]
    fn test_ewp_override_defaults() {
        let mut context = Context::new();

        let id = context
            .add_entity(
                Creature::new()
                    .creature_age(10_u8)
                    .creature_alive(false)
                    .creature_kind(CreatureKind::Bird)
                    .build()
                    .unwrap(),
            )
            .unwrap();

        let alive: bool = context.get_property::<_, CreatureAlive>(id);
        assert!(!alive);

        let kind: CreatureKind = context.get_property::<_, Property<CreatureKind>>(id);
        assert_eq!(kind, CreatureKind::Bird);
    }

    #[test]
    fn test_ewp_full_builder_flow() {
        let mut context = Context::new();

        let id = context
            .add_entity(
                Creature::new()
                    .creature_age(42_u8)
                    .creature_alive(true)
                    .creature_kind(CreatureKind::Dog)
                    .build()
                    .unwrap(),
            )
            .unwrap();

        assert_eq!(context.get_property::<Creature, CreatureAge>(id), 42_u8);
        assert!(context.get_property::<Creature, CreatureAlive>(id));
        assert_eq!(
            context.get_property::<Creature, Property<CreatureKind>>(id),
            CreatureKind::Dog
        );
    }

    #[test]
    fn test_ewp_property_change_event() {
        use crate::entity::events::PropertyChangeEvent;

        type KindChange = PropertyChangeEvent<Creature, Property<CreatureKind>>;

        let mut context = Context::new();

        define_data_plugin!(KindEventCount, usize, 0);

        context.subscribe_to_event::<KindChange>(move |context, event| {
            assert_eq!(event.previous, CreatureKind::Cat);
            assert_eq!(event.current, CreatureKind::Dog);
            *context.get_data_mut(KindEventCount) += 1;
        });

        let id = context
            .add_entity(Creature::new().creature_age(1_u8).build().unwrap())
            .unwrap();
        context.set_property::<_, Property<CreatureKind>>(id, CreatureKind::Dog);
        context.execute();

        assert_eq!(*context.get_data(KindEventCount), 1);
    }

    // === Tests for optional trailing comma ===

    // define_entity! without trailing comma
    define_property!(NoCommaAge, u8);
    define_property!(NoCommaWeight, f64);
    define_property!(
        enum NoCommaStatus {
            Active,
            Inactive,
        }
    );

    define_entity!(struct Widget {
        NoCommaAge,
        NoCommaWeight = 0.0,
        Property<NoCommaStatus> = NoCommaStatus::Active
    });

    #[test]
    fn test_define_entity_no_trailing_comma() {
        let mut context = Context::new();

        let id = context
            .add_entity(Widget::new().no_comma_age(5_u8).build().unwrap())
            .unwrap();

        let age: u8 = context.get_property::<_, NoCommaAge>(id);
        assert_eq!(age, 5);

        let weight: f64 = context.get_property::<_, NoCommaWeight>(id);
        assert_eq!(weight, 0.0);

        let status: NoCommaStatus = context.get_property::<_, Property<NoCommaStatus>>(id);
        assert_eq!(status, NoCommaStatus::Active);
    }

    // define_entity_with_properties! without trailing comma
    define_property!(GadgetSize, u32);
    define_property!(GadgetEnabled, bool);

    define_entity_with_properties!(Gadget {
        GadgetSize,
        GadgetEnabled = true
    });

    #[test]
    fn test_define_entity_with_properties_no_trailing_comma() {
        let mut context = Context::new();

        let id = context
            .add_entity(Gadget::new().gadget_size(42_u32).build().unwrap())
            .unwrap();

        let size: u32 = context.get_property::<_, GadgetSize>(id);
        assert_eq!(size, 42);

        let enabled: bool = context.get_property::<_, GadgetEnabled>(id);
        assert!(enabled);
    }
}
