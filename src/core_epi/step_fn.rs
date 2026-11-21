/// A piecewise constant function of time: `initial` until the first step,
/// then each step's value from its time on. Built with
/// [`StepFn::new(initial).step_at(t, v)`](StepFn::step_at); times must be
/// increasing.
#[derive(Clone, Debug, PartialEq)]
pub struct StepFn {
    initial: f64,
    steps: Vec<(f64, f64)>,
}

impl StepFn {
    pub fn new(initial: f64) -> Self {
        Self {
            initial,
            steps: Vec::new(),
        }
    }

    /// Value `v` from time `t` on.
    ///
    /// # Panics
    ///
    /// If `t` is not finite or not after the previous step.
    #[must_use]
    pub fn step_at(mut self, t: f64, v: f64) -> Self {
        assert!(t.is_finite(), "step time must be finite, got {t}");
        if let Some(&(last, _)) = self.steps.last() {
            assert!(
                t > last,
                "step at {t} is not after the previous step at {last}"
            );
        }
        self.steps.push((t, v));
        self
    }

    /// The value at time `t`.
    pub fn at(&self, t: f64) -> f64 {
        self.steps
            .iter()
            .rev()
            .find(|&&(start, _)| start <= t)
            .map_or(self.initial, |&(_, v)| v)
    }
}

#[cfg(test)]
mod tests {
    use super::StepFn;

    #[test]
    fn value_holds_from_each_step() {
        let f = StepFn::new(1.0).step_at(1.0, 0.5).step_at(3.0, 0.25);
        assert_eq!(f.at(0.0), 1.0);
        assert_eq!(f.at(0.999), 1.0);
        assert_eq!(f.at(1.0), 0.5);
        assert_eq!(f.at(2.0), 0.5);
        assert_eq!(f.at(3.0), 0.25);
        assert_eq!(f.at(100.0), 0.25);
    }

    #[test]
    #[should_panic(expected = "not after")]
    fn steps_must_increase() {
        let _ = StepFn::new(1.0).step_at(2.0, 0.5).step_at(2.0, 0.25);
    }

    #[test]
    #[should_panic(expected = "must be finite")]
    fn first_step_rejects_nan() {
        let _ = StepFn::new(1.0).step_at(f64::NAN, 0.5);
    }
}
