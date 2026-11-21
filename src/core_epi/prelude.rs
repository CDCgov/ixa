//! Every item `core_epi` exports plus the `define_modifier!` and
//! `define_settings!` macros, for a glob import.
//!
//! ```
//! use ixa::core_epi::prelude::*;
//! use ixa::prelude::*;
//!
//! define_entity!(Person);
//! define_settings!(Setting for Person { Household });
//! define_modifier!(Mask);
//!
//! struct Flu;
//!
//! impl InfectionManager<Flu> for Context {
//!     type Entity = Person;
//!     type Details = ();
//!
//!     fn draw_infectiousness_rate_fn(&mut self, _person: PersonId, _details: &()) -> RateAssignment {
//!         ConstantRateFn::new(0.4, 5.0).into()
//!     }
//! }
//!
//! impl TransmissionManager<Flu> for Context {}
//!
//! let mut context = Context::new();
//! context.init_random(42);
//! let person = context.add_entity(Person).unwrap();
//! context.set_setting_alpha(Setting::Household, 1.0);
//! context.set_infectiousness_modifier(person, Mask, 0.5);
//! context.setup_transmission_loop();
//! context.seed_infection(person);
//! ```

pub use super::{
    AnyInfectionRate, ConstantRateFn, EmpiricalRateFn, InfectionManager,
    InfectiousnessAssignedEvent, InvertibleRate, Itinerary, ItineraryAssignment, ModifierId,
    ModifierKind, ModifiersExt, PastInfection, RateAssignment, SettingGroupId, SettingKind,
    SettingsExt, StepFn, TransformedInvertibleRateFn, TransmissionManager, MAX_SETTINGS,
};
pub use crate::{define_modifier, define_settings};
