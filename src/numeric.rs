#![allow(clippy::approx_constant)]
//! Vendored from [statrs@0.18.0 (prec.rs)](http://github.com/statrs-dev/statrs/blob/v0.18.0/src/prec.rs), convenience
//! wrappers around methods from the approx crate. Provides utility functions for working with floating point precision.

use approx::AbsDiffEq;

/// Targeted accuracy instantiated over `f64`
pub const ACC: f64 = 10e-11;

/// Compares if two floats are close via `approx::abs_diff_eq` using a maximum absolute difference
/// (epsilon) of `acc`.
#[must_use]
pub fn almost_eq(a: f64, b: f64, acc: f64) -> bool {
    if a.is_infinite() && b.is_infinite() {
        return a == b;
    }
    a.abs_diff_eq(&b, acc)
}

/// Compares if two floats are close via `approx::relative_eq!` and `ACC` relative precision.
/// Updates first argument to value of second argument.
#[must_use]
pub fn convergence(x: &mut f64, x_new: f64) -> bool {
    let res = approx::relative_eq!(*x, x_new, max_relative = ACC);
    *x = x_new;
    res
}

// Not from statrs.
#[cfg(test)]
mod tests {
    use super::*;
    use crate::assert_almost_eq;

    #[test]
    fn almost_eq_within_tolerance() {
        let a = 1.0;
        let b = 1.0 + 0.5e-11;
        // within ACC = 10e-11
        assert!(almost_eq(a, b, ACC));
    }

    #[test]
    fn almost_eq_outside_tolerance() {
        let a = 1.0;
        let b = 1.0 + 2e-10;
        // 2e-10 > 10e-11
        assert!(!almost_eq(a, b, ACC));
    }

    #[test]
    fn almost_eq_infinities() {
        assert!(almost_eq(f64::INFINITY, f64::INFINITY, ACC));
        assert!(almost_eq(f64::NEG_INFINITY, f64::NEG_INFINITY, ACC));
        assert!(!almost_eq(f64::INFINITY, f64::NEG_INFINITY, ACC));
    }

    #[test]
    fn convergence_updates_and_compares() {
        let mut x = 100.0;
        // first call: compare 100.0 vs 100.0 → exactly equal → true
        assert!(convergence(&mut x, 100.0));
        // x should now be updated
        assert_eq!(x, 100.0);

        // now pick a new value within relative ACC
        let x_new = x * (1.0 + 0.5 * ACC);
        assert!(convergence(&mut x, x_new));
        assert_eq!(x, x_new);

        // now pick something well outside relative ACC
        let x_new2 = x * (1.0 + 2.0 * ACC);
        assert!(!convergence(&mut x, x_new2));
        assert_eq!(x, x_new2);
    }

    #[test]
    fn assert_almost_eq_macro_passes() {
        // should not panic
        assert_almost_eq!(3.14159265, 3.14159264, 1e-7);
    }

    #[test]
    #[should_panic(expected = "assertion failed")]
    fn assert_almost_eq_macro_panics() {
        // difference is 1e-3, but prec=1e-4 → panic
        assert_almost_eq!(1.0, 1.001, 1e-4);
    }
}
