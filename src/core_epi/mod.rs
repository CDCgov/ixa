//! Common epidemiological modeling components.
//!
//! The model-agnostic pieces of a transmission model: the [`InvertibleRate`]
//! interface, its combinators, and the
//! concrete rate functions ([`rate`]), per-person rate assignment, storage,
//! and infectiousness modifiers ([`infection_manager`]), and the generic NHPP
//! transmission loop with its forecast contract ([`transmission_manager`]). A
//! model plugs in by implementing [`InfectionManager`] and
//! [`TransmissionManager`] on `Context`, parameterized by a marker type of its
//! own, and naming its own entity (`type Entity = Person;`) as the one that is
//! infected.

pub mod infection_manager;
pub mod rate;
pub mod settings;
pub mod step_fn;
pub mod transmission_manager;

pub use infection_manager::{
    InfectionManager, InfectiousnessAssignedEvent, ModifierFn, RateAssignment,
};
pub use rate::{
    AnyInfectionRate, ConstantRateFn, EmpiricalRateFn, InvertibleRate, ScaledRateFn,
    TransformedInvertibleRateFn,
};
pub use settings::{
    IntoItineraryAssignment, Itinerary, ItineraryAssignment, SettingGroupId, SettingKind,
    SettingsExt, MAX_SETTINGS,
};
pub use step_fn::StepFn;
pub use transmission_manager::TransmissionManager;
