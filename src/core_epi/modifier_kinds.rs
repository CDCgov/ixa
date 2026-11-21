//! Modifier kinds: the keys of infectiousness and susceptibility modifiers.

use std::any::TypeId;

use crate::{define_data_plugin, Context, ContextBase};

/// What keys a modifier: a type declared with
/// [`define_modifier!`](crate::define_modifier), or a [`ModifierId`] added at
/// run time.
pub trait ModifierKind: Copy + 'static {
    /// One key per type unless overridden, so every value of a declared kind
    /// is the same modifier.
    #[doc(hidden)]
    fn key(self) -> ModifierKey {
        ModifierKey(Key::Declared(TypeId::of::<Self>()))
    }
}

/// Identifies a modifier kind, declared or added at run time.
#[doc(hidden)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ModifierKey(Key);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Key {
    Declared(TypeId),
    RunTime(u32),
}

/// Declares a modifier kind: a unit type passed by value to the modifier
/// methods of [`InfectionManager`](crate::core_epi::InfectionManager). Each
/// module can declare its own.
///
/// ```
/// ixa::define_modifier!(pub Mask);
/// let kind = Mask;
/// ```
#[macro_export]
macro_rules! define_modifier {
    ($vis:vis $name:ident) => {
        #[derive(Clone, Copy, Debug, PartialEq, Eq)]
        $vis struct $name;

        impl $crate::core_epi::ModifierKind for $name {}
    };
}

/// A modifier kind added at run time with
/// [`add_modifier_kind`](ModifiersExt::add_modifier_kind), e.g. for an
/// intervention named in a parameters file.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ModifierId(u32);

impl ModifierKind for ModifierId {
    fn key(self) -> ModifierKey {
        ModifierKey(Key::RunTime(self.0))
    }
}

// The names of the kinds added at run time, by `ModifierId`.
define_data_plugin!(ModifierKindsPlugin, Vec<String>, Vec::new());

/// Modifier kinds added at run time. They share one set of names per
/// context, whichever model uses them.
pub trait ModifiersExt: ContextBase {
    /// Add a kind of modifier called `name`. Kinds can be added at any time.
    ///
    /// ```
    /// use ixa::core_epi::ModifiersExt;
    /// use ixa::prelude::*;
    ///
    /// let mut context = Context::new();
    /// let ventilation = context.add_modifier_kind("ventilation");
    /// assert_eq!(context.modifier_kind_by_name("ventilation"), Some(ventilation));
    /// assert_eq!(context.modifier_name(ventilation), "ventilation");
    /// assert_eq!(context.modifier_kinds(), [ventilation]);
    /// ```
    ///
    /// # Panics
    ///
    /// If the name is taken.
    fn add_modifier_kind(&mut self, name: &str) -> ModifierId {
        let names = self.get_data_mut(ModifierKindsPlugin);
        assert!(
            names.iter().all(|existing| existing != name),
            "modifier kind {name} already exists"
        );
        let id = ModifierId(u32::try_from(names.len()).expect("too many modifier kinds"));
        names.push(name.to_string());
        id
    }

    /// The kinds added so far, in order.
    fn modifier_kinds(&self) -> Vec<ModifierId> {
        let count = self.get_data(ModifierKindsPlugin).len();
        (0..count as u32).map(ModifierId).collect()
    }

    /// The kind added as `name`, if there is one.
    fn modifier_kind_by_name(&self, name: &str) -> Option<ModifierId> {
        let index = self
            .get_data(ModifierKindsPlugin)
            .iter()
            .position(|existing| existing == name)?;
        Some(ModifierId(index as u32))
    }

    /// The name `modifier` was added with.
    fn modifier_name(&self, modifier: ModifierId) -> &str {
        &self.get_data(ModifierKindsPlugin)[modifier.0 as usize]
    }
}

impl ModifiersExt for Context {}
