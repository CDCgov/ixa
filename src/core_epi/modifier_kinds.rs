//! Modifier kinds: the keys of infectiousness and susceptibility modifiers.

use std::any::TypeId;

use crate::{define_data_plugin, Context, ContextBase};

/// What keys a modifier: a type declared with
/// [`define_modifier!`](crate::define_modifier), or a [`ModifierId`] added at
/// run time.
///
/// A declared kind is identified by its type, so an implementation written by
/// hand must be on a type with no data. One with several values would make
/// them all the same modifier, and does not compile:
///
/// ```compile_fail
/// use ixa::core_epi::ModifierKind;
///
/// #[derive(Clone, Copy)]
/// enum Intervention {
///     Mask,
///     Ventilation,
/// }
/// impl ModifierKind for Intervention {}
/// let key = Intervention::Mask.key();
/// ```
pub trait ModifierKind: Copy + 'static {
    #[doc(hidden)]
    fn key(self) -> ModifierKey {
        const {
            assert!(
                std::mem::size_of::<Self>() == 0,
                "a modifier kind must be a type with no data: declare one with define_modifier!"
            );
        }
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
/// ixa::define_modifier!(
///     /// Wearing a mask.
///     pub Mask
/// );
/// let kind = Mask;
/// ```
#[macro_export]
macro_rules! define_modifier {
    ($(#[$attribute:meta])* $vis:vis $name:ident) => {
        $(#[$attribute])*
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
        $vis struct $name;

        impl $crate::core_epi::ModifierKind for $name {}
    };
}

/// A modifier kind added at run time with
/// [`add_modifier_kind`](ModifiersExt::add_modifier_kind), e.g. for an
/// intervention named in a parameters file. It belongs to the context that
/// made it: on another context it is whichever kind was added in the same
/// position there, so do not keep one across runs.
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
    /// If the name is taken: names are shared by the whole context, so add
    /// each kind in one place and look it up elsewhere with
    /// [`modifier_kind_by_name`](Self::modifier_kind_by_name).
    fn add_modifier_kind(&mut self, name: &str) -> ModifierId {
        assert!(
            self.modifier_kind_by_name(name).is_none(),
            "modifier kind {name} already exists"
        );
        let names = self.get_data_mut(ModifierKindsPlugin);
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
    ///
    /// # Panics
    ///
    /// If `modifier` is from another context and this one has no kind in
    /// its position.
    fn modifier_name(&self, modifier: ModifierId) -> &str {
        match self.get_data(ModifierKindsPlugin).get(modifier.0 as usize) {
            Some(name) => name,
            None => panic!("{modifier:?} was not added to this context"),
        }
    }
}

impl ModifiersExt for Context {}
