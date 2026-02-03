//! Structs representing queries with a fixed number of property values.
//!
//! These structs are used instead of tuples to provide better type safety
//! and enable entity-specific query macros.

use std::fmt::Debug;
use std::marker::PhantomData;

use crate::entity::property::Property;
use crate::entity::Entity;

/// A query with no properties (matches all entities).
pub struct PropertyEntityValues0<E: Entity> {
    // Using fn() -> E avoids requiring E: Copy
    _marker: PhantomData<fn() -> E>,
}

// Manual Copy/Clone/Debug impls to avoid bounds on E
impl<E: Entity> Copy for PropertyEntityValues0<E> {}
impl<E: Entity> Clone for PropertyEntityValues0<E> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<E: Entity> Debug for PropertyEntityValues0<E> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PropertyEntityValues0").finish()
    }
}

impl<E: Entity> PropertyEntityValues0<E> {
    #[allow(clippy::new_without_default)]
    pub fn new() -> Self {
        Self {
            _marker: PhantomData,
        }
    }
}

/// A query with one property.
pub struct PropertyEntityValues1<E: Entity, P0: Property<E>> {
    pub _0: P0,
    // Using fn() -> E avoids requiring E: Copy
    _marker: PhantomData<fn() -> E>,
}

// Manual Copy/Clone/Debug impls to avoid bounds on E
impl<E: Entity, P0: Property<E>> Copy for PropertyEntityValues1<E, P0> {}
impl<E: Entity, P0: Property<E>> Clone for PropertyEntityValues1<E, P0> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<E: Entity, P0: Property<E>> Debug for PropertyEntityValues1<E, P0> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PropertyEntityValues1")
            .field("_0", &self._0)
            .finish()
    }
}

impl<E: Entity, P0: Property<E>> PropertyEntityValues1<E, P0> {
    pub fn new(p0: P0) -> Self {
        Self {
            _0: p0,
            _marker: PhantomData,
        }
    }
}

macro_rules! define_property_entity_values {
    ($name:ident, $ct:expr, $($idx:tt: $param:ident),+) => {
        #[doc = concat!("A query with ", stringify!($ct), " properties.")]
        pub struct $name<E: Entity, $($param: Property<E>),+> {
            $(pub $idx: $param,)+
            // Using fn() -> E avoids requiring E: Copy
            _marker: PhantomData<fn() -> E>,
        }

        // Manual Copy/Clone/Debug impls to avoid bounds on E
        impl<E: Entity, $($param: Property<E>),+> Copy for $name<E, $($param),+> {}
        impl<E: Entity, $($param: Property<E>),+> Clone for $name<E, $($param),+> {
            fn clone(&self) -> Self {
                *self
            }
        }
        impl<E: Entity, $($param: Property<E>),+> Debug for $name<E, $($param),+> {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.debug_struct(stringify!($name))
                    $(.field(stringify!($idx), &self.$idx))+
                    .finish()
            }
        }

        impl<E: Entity, $($param: Property<E>),+> $name<E, $($param),+> {
            #[allow(clippy::just_underscores_and_digits, clippy::too_many_arguments)]
            pub fn new($($idx: $param),+) -> Self {
                Self {
                    $($idx,)+
                    _marker: PhantomData,
                }
            }
        }
    };
}

define_property_entity_values!(PropertyEntityValues2, 2, _0: P0, _1: P1);
define_property_entity_values!(PropertyEntityValues3, 3, _0: P0, _1: P1, _2: P2);
define_property_entity_values!(PropertyEntityValues4, 4, _0: P0, _1: P1, _2: P2, _3: P3);
define_property_entity_values!(PropertyEntityValues5, 5, _0: P0, _1: P1, _2: P2, _3: P3, _4: P4);
define_property_entity_values!(PropertyEntityValues6, 6, _0: P0, _1: P1, _2: P2, _3: P3, _4: P4, _5: P5);
define_property_entity_values!(PropertyEntityValues7, 7, _0: P0, _1: P1, _2: P2, _3: P3, _4: P4, _5: P5, _6: P6);
define_property_entity_values!(PropertyEntityValues8, 8, _0: P0, _1: P1, _2: P2, _3: P3, _4: P4, _5: P5, _6: P6, _7: P7);
define_property_entity_values!(PropertyEntityValues9, 9, _0: P0, _1: P1, _2: P2, _3: P3, _4: P4, _5: P5, _6: P6, _7: P7, _8: P8);
define_property_entity_values!(PropertyEntityValues10, 10, _0: P0, _1: P1, _2: P2, _3: P3, _4: P4, _5: P5, _6: P6, _7: P7, _8: P8, _9: P9);
