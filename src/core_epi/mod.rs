//! Common epidemiological modeling components.
//!
//! Model-agnostic building blocks for transmission models:
//! - [`rate`]: the [`InvertibleRate`] interface, combinators, and concrete rate
//!   functions.
//! - [`infection_manager`]: per-person rate assignment, storage, and
//!   infectiousness or susceptibility modifiers.
//! - [`modifier_kinds`]: the kinds that key those modifiers, declared by a
//!   module or added at run time.
//! - [`transmission_manager`]: the continuous-time NHPP transmission loop and
//!   its forecast contract.
//! - [`settings`]: contact groups (such as households) with per-person
//!   itineraries.
//! - [`StepFn`]: piecewise-constant modifier factors.
//!
//! A model plugs in by implementing [`InfectionManager`] and
//! [`TransmissionManager`] on `Context`, parameterized by its own marker type
//! and target entity (`type Entity = Person;`).

pub mod infection_manager;
pub mod modifier_kinds;
pub mod rate;
pub mod settings;
pub mod step_fn;
pub mod transmission_manager;

pub use infection_manager::{
    InfectionManager, InfectiousnessAssignedEvent, PastInfection, RateAssignment,
};
pub use modifier_kinds::{ModifierId, ModifierKind, ModifiersExt};
pub use rate::{
    AnyInfectionRate, ConstantRateFn, EmpiricalRateFn, InvertibleRate, TransformedInvertibleRateFn,
};
pub use settings::{
    Itinerary, ItineraryAssignment, SettingGroupId, SettingKind, SettingsExt, MAX_SETTINGS,
};
pub use step_fn::StepFn;
pub use transmission_manager::TransmissionManager;
