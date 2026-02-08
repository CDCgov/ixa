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
/// ## Entity with associated properties
/// ```rust,ignore
/// define_entity!(struct Person {
///     Age,                                       // required property
///     Weight = 0.0,                              // optional with default
///     Property<InfectionStatus> = InfectionStatus::Susceptible,  // enum property with default
/// });
/// ```
///
/// The property-bearing form:
/// - Generates `impl PropertyDef<Person>` for each listed property
/// - Generates a `PersonBuilder` struct with setter methods
/// - `Person::build()` returns a `PersonBuilder`
/// - The builder implements `PropertyList<Person>` for use with `add_entity`
#[macro_export]
macro_rules! define_entity {
    // Simple form (no properties)
    ($entity_name:ident) => {
        #[allow(unused)]
        #[derive(Default, Debug, PartialEq, Eq, Clone, Copy)]
        pub struct $entity_name;

        impl $entity_name {
            #[allow(unused)]
            pub fn new() -> Self {
                Self::default()
            }
        }

        $crate::impl_entity!($entity_name);
    };

    // Form with property declarations
    (struct $entity_name:ident { $($prop_decl:tt)* }) => {
        // Define the entity struct and implement Entity trait
        $crate::define_entity!($entity_name);

        // Process property declarations: accumulates (prop_ty, field_name, value_ty) into builder list
        // and emits impl_property_for_entity! calls eagerly.
        $crate::define_entity!(@process_props $entity_name, [], $($prop_decl)*);
    };

    // === Internal rules for parsing property declarations ===

    // Terminal: no more properties to parse. Generate builder.
    (@process_props $entity:ident,
        [$( ($all_prop:ty, $all_field:ident, $all_value_ty:ty) )*],
    ) => {
        // Generate the builder struct and impl
        $crate::define_entity!(@generate_builder $entity,
            [$( ($all_prop, $all_field, $all_value_ty) )*]
        );
    };

    // Parse a required property: `PropType,`
    (@process_props $entity:ident,
        [$($all:tt)*],
        $prop:ident, $($rest:tt)*
    ) => {
        $crate::impl_property_for_entity!($prop, $entity);
        $crate::define_entity!(@process_props $entity,
            [$($all)* ($prop, $prop, <$prop as $crate::entity::property::IsProperty>::Value)],
            $($rest)*
        );
    };

    // Parse a required Property<T>: `Property<T>,`
    (@process_props $entity:ident,
        [$($all:tt)*],
        Property<$inner:ident>, $($rest:tt)*
    ) => {
        $crate::define_entity!(@emit_property_for_entity $entity, $inner);
        $crate::define_entity!(@process_props $entity,
            [$($all)* ($crate::entity::property::Property<$inner>, $inner, $inner)],
            $($rest)*
        );
    };

    // Parse an optional property with default: `PropType = default_value,`
    (@process_props $entity:ident,
        [$($all:tt)*],
        $prop:ident = $default:expr, $($rest:tt)*
    ) => {
        $crate::impl_property_for_entity!($prop, $entity, default_const = $default);
        $crate::define_entity!(@process_props $entity,
            [$($all)* ($prop, $prop, <$prop as $crate::entity::property::IsProperty>::Value)],
            $($rest)*
        );
    };

    // Parse an optional Property<T> with default: `Property<T> = default_value,`
    (@process_props $entity:ident,
        [$($all:tt)*],
        Property<$inner:ident> = $default:expr, $($rest:tt)*
    ) => {
        $crate::define_entity!(@emit_property_for_entity $entity, $inner, default_const = $default);
        $crate::define_entity!(@process_props $entity,
            [$($all)* ($crate::entity::property::Property<$inner>, $inner, $inner)],
            $($rest)*
        );
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

    // Generate builder struct and implementation
    (@generate_builder $entity:ident,
        [$( ($prop:ty, $field:ident, $value_ty:ty) )*]
    ) => {
        $crate::paste::paste! {
            /// Builder for creating entities with explicit property values.
            #[derive(Debug, Clone, Copy)]
            pub struct [<$entity Builder>] {
                $(
                    [<$field:snake>]: Option<$value_ty>,
                )*
            }

            impl $entity {
                /// Create a new builder for this entity type.
                #[allow(unused)]
                pub fn build() -> [<$entity Builder>] {
                    [<$entity Builder>] {
                        $(
                            [<$field:snake>]: None,
                        )*
                    }
                }
            }

            impl [<$entity Builder>] {
                $(
                    #[allow(unused)]
                    pub fn [<$field:snake>](mut self, value: $value_ty) -> Self {
                        self.[<$field:snake>] = Some(value);
                        self
                    }
                )*
            }

            impl $crate::entity::property_list::PropertyList<$entity> for [<$entity Builder>] {
                fn validate() -> Result<(), String> {
                    Ok(())
                }

                fn contains_properties(property_type_ids: &[std::any::TypeId]) -> bool {
                    let self_type_ids: &[std::any::TypeId] = &[
                        $(
                            <$prop as $crate::entity::property::PropertyDef<$entity>>::type_id(),
                        )*
                    ];
                    property_type_ids.iter().all(|id| self_type_ids.contains(id))
                }

                fn contains_required_properties() -> bool {
                    // The builder always contains all properties, so it always satisfies required properties
                    true
                }

                fn set_values_for_entity(
                    &self,
                    entity_id: $crate::entity::EntityId<$entity>,
                    property_store: &$crate::entity::property_store::PropertyStore<$entity>,
                ) {
                    $(
                        if let Some(value) = self.[<$field:snake>] {
                            let store = property_store.get::<$prop>();
                            store.set(entity_id, value);
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
        $crate::define_entity!(@process_props $entity_name, [], $($prop_decl)*);
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
            .add_entity(Animal::build().zst_age(10_u8).zst_weight(55.5))
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

        let id = context.add_entity(Animal::build().zst_age(5_u8)).unwrap();

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
        let id = context.add_entity(Animal::build().zst_age(1_u8)).unwrap();

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

        let id = context.add_entity(Animal::build().zst_age(3_u8)).unwrap();
        context.set_property::<_, Property<ZstStatus>>(id, ZstStatus::I);
        context.execute();

        assert_eq!(*context.get_data(EventCount), 1);
    }

    #[test]
    fn test_marker_based_get_set() {
        let mut context = Context::new();

        let id = context.add_entity(Animal::build().zst_age(5_u8)).unwrap();

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
        let car = context.add_entity(Vehicle::build().speed(60.0)).unwrap();

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
}
