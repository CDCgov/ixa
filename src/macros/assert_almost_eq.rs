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
