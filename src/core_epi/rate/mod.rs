//! Infectiousness rate functions: the [`InvertibleRate`] interface, its
//! combinators, the concrete [`ConstantRateFn`] and [`EmpiricalRateFn`], and
//! [`AnyInfectionRate`] over both.

pub mod any_rate;
pub mod constant_rate;
pub mod empirical_rate;
pub mod rate_fn;

pub use any_rate::AnyInfectionRate;
pub use constant_rate::ConstantRateFn;
pub use empirical_rate::{
    empirical_cum_rate, empirical_curve_duration, empirical_inverse_cum_rate,
    empirical_support_end, EmpiricalRateFn,
};
pub use rate_fn::{InvertibleRate, ScaledRateFn, TransformedInvertibleRateFn};
