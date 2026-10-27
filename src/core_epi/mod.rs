//! Common epidemiological modeling components.
//!
//! The model-agnostic pieces of a transmission model: the [`Person`] entity
//! ([`person`]), the [`InvertibleRate`] interface, its combinators, and the
//! concrete rate functions ([`rate`]), per-person rate assignment, storage,
//! and infectiousness modifiers ([`infection_manager`]), and the generic NHPP
//! transmission loop and forecast contract ([`transmission`]). A model plugs in by implementing
//! [`InfectionManager`] and [`Transmission`] on `Context`,
//! parameterized by a marker type of its own.

pub mod infection_manager;
pub mod person;
pub mod rate;
pub mod transmission;

pub use infection_manager::{InfectionManager, InfectiousnessAssignedEvent, RateAssignment};
pub use person::{Person, PersonId};
pub use rate::{
    AnyInfectionRate, ConstantRateFn, EmpiricalRateFn, InvertibleRate, ScaledRateFn,
    TransformedInvertibleRateFn,
};
pub use transmission::Transmission;
