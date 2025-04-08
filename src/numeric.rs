//! Vendored from statrs@0.18.0 (prec.rs), convenience wrappers around methods from the approx crate.
//! Provides utility functions for working with floating point precision

use approx::AbsDiffEq;

/// Targeted accuracy instantiated over `f64`
pub const ACC: f64 = 10e-11;

/// Compares if two floats are close via `approx::abs_diff_eq` using a maximum absolute difference
/// (epsilon) of `acc`.
pub fn almost_eq(a: f64, b: f64, acc: f64) -> bool {
    if a.is_infinite() && b.is_infinite() {
        return a == b;
    }
    a.abs_diff_eq(&b, acc)
}

/// Compares if two floats are close via `approx::relative_eq!` and `ACC` relative precision.
/// Updates first argument to value of second argument.
pub fn convergence(x: &mut f64, x_new: f64) -> bool {
    let res = approx::relative_eq!(*x, x_new, max_relative = ACC);
    *x = x_new;
    res
}

#[macro_export]
macro_rules! assert_almost_eq {
    ($a:expr, $b:expr, $prec:expr $(,)?) => {
        if !$crate::numeric::almost_eq($a, $b, $prec) {
            panic!(
                "assertion failed: `abs(left - right) < {:e}`, (left: `{}`, right: `{}`)",
                $prec, $a, $b
            );
        }
    };
}
