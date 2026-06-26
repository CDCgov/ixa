use std::any::TypeId;
use std::cell::RefMut;
use std::hash::Hasher;

use log::trace;

use crate::hashing::{hash_str, DeterministicHasher};
use crate::rand::distr::uniform::{SampleRange, SampleUniform};
use crate::rand::distr::weighted::{Weight, WeightedIndex};
use crate::rand::distr::Distribution;
use crate::rand::{Rng, RngCore, SeedableRng};
use crate::random::{RngHolder, RngPlugin};
use crate::{Context, ContextBase, RngId};

/// Gets a mutable reference to the random number generator associated with the given
/// [`RngId`]. If the Rng has not been used before, one will be created with the base seed
/// you defined in `init`. Note that this will panic if `init` was not called yet.
fn get_rng<R: RngId + 'static>(context: &impl ContextBase) -> RefMut<R::RngType> {
    let data_container = context.get_data(RngPlugin);

    let rng_holders = data_container.rng_holders.try_borrow_mut().unwrap();
    RefMut::map(rng_holders, |holders| {
        holders
            .entry(TypeId::of::<R>())
            // Create a new rng holder if it doesn't exist yet
            .or_insert_with(|| {
                trace!(
                    "creating new RNG (seed={}) for type id {:?}",
                    data_container.base_seed,
                    TypeId::of::<R>()
                );
                let base_seed = data_container.base_seed;
                let seed_offset = hash_str(R::get_name());
                RngHolder {
                    rng: Box::new(R::RngType::seed_from_u64(
                        base_seed.wrapping_add(seed_offset),
                    )),
                }
            })
            .rng
            .downcast_mut::<R::RngType>()
            .unwrap()
    })
}

// This is a trait extension on Context for
// random number generation functionality.
pub trait ContextRandomExt: ContextBase {
    /// Initializes the `RngPlugin` data container to store rngs as well as a base
    /// seed. Note that rngs are created lazily when `get_rng` is called.
    fn init_random(&mut self, base_seed: u64) {
        trace!("initializing random module");
        let data_container = self.get_data_mut(RngPlugin);
        data_container.base_seed = base_seed;

        // Clear any existing Rngs to ensure they get re-seeded when `get_rng` is called
        let mut rng_map = data_container.rng_holders.try_borrow_mut().unwrap();
        rng_map.clear();
    }

    /// Gets a random sample from the random number generator associated with the given
    /// [`RngId`] by applying the specified sampler function. If the Rng has not been used
    /// before, one will be created with the base seed you defined in `set_base_random_seed`.
    /// Note that this will panic if `set_base_random_seed` was not called yet.
    #[must_use]
    fn sample<R: RngId + 'static, T>(
        &self,
        _rng_type: R,
        sampler: impl FnOnce(&mut R::RngType) -> T,
    ) -> T {
        let mut rng = get_rng::<R>(self);
        sampler(&mut rng)
    }

    /// Returns a deterministic debug fingerprint for the current position of the random number
    /// generator associated with the given [`RngId`].
    ///
    /// This clones the current generator and hashes a small number of draws from the clone, so it
    /// does not advance the generator used by sampling. The returned value is intended only for
    /// comparing checkpoints across runs of the same code and dependency versions. It is not a
    /// serialization format, and it does not guarantee reproducibility across RNG algorithm or
    /// version changes.
    #[must_use]
    fn debug_rng_state<R: RngId + 'static>(&self, _rng_id: R) -> u64
    where
        R::RngType: Clone + RngCore,
    {
        let rng = get_rng::<R>(self);
        let mut rng_snapshot = (*rng).clone();
        let mut hasher = DeterministicHasher::default();

        for _ in 0..4 {
            hasher.write(&rng_snapshot.next_u64().to_le_bytes());
        }

        hasher.finish()
    }

    /// Gets a random sample from the specified distribution using a random number generator
    /// associated with the given [`RngId`]. If the Rng has not been used before, one will be
    /// created with the base seed you defined in `set_base_random_seed`.
    /// Note that this will panic if `set_base_random_seed` was not called yet.
    #[must_use]
    fn sample_distr<R: RngId + 'static, T>(
        &self,
        _rng_type: R,
        distribution: impl Distribution<T>,
    ) -> T
    where
        R::RngType: Rng,
    {
        let mut rng = get_rng::<R>(self);
        distribution.sample::<R::RngType>(&mut rng)
    }

    /// Gets a random sample within the range provided by `range`
    /// using the generator associated with the given [`RngId`].
    /// Note that this will panic if `set_base_random_seed` was not called yet.
    #[must_use]
    fn sample_range<R: RngId + 'static, S, T>(&self, rng_id: R, range: S) -> T
    where
        R::RngType: Rng,
        S: SampleRange<T>,
        T: SampleUniform,
    {
        self.sample(rng_id, |rng| rng.random_range(range))
    }

    /// Gets a random boolean value which is true with probability `p`
    /// using the generator associated with the given [`RngId`].
    /// Note that this will panic if `set_base_random_seed` was not called yet.
    #[must_use]
    fn sample_bool<R: RngId + 'static>(&self, rng_id: R, p: f64) -> bool
    where
        R::RngType: Rng,
    {
        self.sample(rng_id, |rng| rng.random_bool(p))
    }

    /// Draws a random entry out of the list provided in `weights`
    /// with the given weights using the generator associated with the
    /// given [`RngId`].  Note that this will panic if
    /// `set_base_random_seed` was not called yet.
    #[must_use]
    fn sample_weighted<R: RngId + 'static, T>(&self, _rng_id: R, weights: &[T]) -> usize
    where
        R::RngType: Rng,
        T: Clone
            + Default
            + SampleUniform
            + for<'a> std::ops::AddAssign<&'a T>
            + PartialOrd
            + Weight,
    {
        let index = WeightedIndex::new(weights).unwrap();
        let mut rng = get_rng::<R>(self);
        index.sample(&mut *rng)
    }
}

impl ContextRandomExt for Context {}

#[cfg(test)]
mod test {
    use crate::context::Context;
    use crate::rand::distr::weighted::WeightedIndex;
    use crate::rand::distr::Distribution;
    use crate::rand::RngCore;
    use crate::random::context_ext::ContextRandomExt;
    use crate::{define_data_plugin, define_rng};

    define_rng!(FooRng);
    define_rng!(BarRng);

    #[test]
    fn get_rng_basic() {
        let mut context = Context::new();
        context.init_random(42);

        assert_ne!(
            context.sample(FooRng, RngCore::next_u64),
            context.sample(FooRng, RngCore::next_u64)
        );
    }

    #[test]
    fn multiple_rng_types() {
        let mut context = Context::new();
        context.init_random(42);

        assert_ne!(
            context.sample(FooRng, RngCore::next_u64),
            context.sample(BarRng, RngCore::next_u64)
        );
    }

    #[test]
    fn reset_seed() {
        let mut context = Context::new();
        context.init_random(42);

        let run_0 = context.sample(FooRng, RngCore::next_u64);
        let run_1 = context.sample(FooRng, RngCore::next_u64);

        // Reset with same seed, ensure we get the same values
        context.init_random(42);
        assert_eq!(run_0, context.sample(FooRng, RngCore::next_u64));
        assert_eq!(run_1, context.sample(FooRng, RngCore::next_u64));

        // Reset with different seed, ensure we get different values
        context.init_random(88);
        assert_ne!(run_0, context.sample(FooRng, RngCore::next_u64));
        assert_ne!(run_1, context.sample(FooRng, RngCore::next_u64));
    }

    #[test]
    fn debug_rng_state_matches_for_same_seed_and_progress() {
        let mut context_0 = Context::new();
        context_0.init_random(42);
        let mut context_1 = Context::new();
        context_1.init_random(42);

        for _ in 0..3 {
            let _ = context_0.sample(FooRng, RngCore::next_u64);
            let _ = context_1.sample(FooRng, RngCore::next_u64);
        }

        assert_eq!(
            context_0.debug_rng_state(FooRng),
            context_1.debug_rng_state(FooRng)
        );
    }

    #[test]
    fn debug_rng_state_changes_with_rng_progress() {
        let mut context = Context::new();
        context.init_random(42);

        let initial = context.debug_rng_state(FooRng);
        let _ = context.sample(FooRng, RngCore::next_u64);

        assert_ne!(initial, context.debug_rng_state(FooRng));
    }

    #[test]
    fn debug_rng_state_is_stable_without_sampling() {
        let mut context = Context::new();
        context.init_random(42);

        assert_eq!(
            context.debug_rng_state(FooRng),
            context.debug_rng_state(FooRng)
        );
    }

    #[test]
    fn debug_rng_state_does_not_affect_next_sample() {
        let mut with_debug = Context::new();
        with_debug.init_random(42);
        let mut without_debug = Context::new();
        without_debug.init_random(42);

        let _ = with_debug.debug_rng_state(FooRng);

        assert_eq!(
            with_debug.sample(FooRng, RngCore::next_u64),
            without_debug.sample(FooRng, RngCore::next_u64)
        );
    }

    #[test]
    fn debug_rng_state_resets_with_seed() {
        let mut context = Context::new();
        context.init_random(42);

        let initial = context.debug_rng_state(FooRng);
        let _ = context.sample(FooRng, RngCore::next_u64);
        assert_ne!(initial, context.debug_rng_state(FooRng));

        context.init_random(42);
        assert_eq!(initial, context.debug_rng_state(FooRng));
    }

    #[test]
    fn debug_rng_state_is_independent_by_rng_id() {
        let mut context_0 = Context::new();
        context_0.init_random(42);
        let mut context_1 = Context::new();
        context_1.init_random(42);

        let foo_initial = context_0.debug_rng_state(FooRng);
        let bar_initial = context_0.debug_rng_state(BarRng);
        assert_ne!(foo_initial, bar_initial);

        let _ = context_0.sample(FooRng, RngCore::next_u64);
        let _ = context_1.sample(BarRng, RngCore::next_u64);

        assert_ne!(context_0.debug_rng_state(FooRng), foo_initial);
        assert_eq!(context_0.debug_rng_state(BarRng), bar_initial);
        assert_eq!(context_1.debug_rng_state(FooRng), foo_initial);
        assert_ne!(context_1.debug_rng_state(BarRng), bar_initial);
    }

    define_data_plugin!(
        SamplerData,
        WeightedIndex<f64>,
        WeightedIndex::new(vec![1.0]).unwrap()
    );

    #[test]
    fn sampler_function_closure_capture() {
        let mut context = Context::new();
        context.init_random(42);

        // Initialize weighted sampler. Zero is selected with probability 1/3, one with a
        // probability of 2/3.
        *context.get_data_mut(SamplerData) = WeightedIndex::new(vec![1.0, 2.0]).unwrap();

        let parameters = context.get_data(SamplerData);
        let n_samples = 3000;
        let mut zero_counter = 0;
        for _ in 0..n_samples {
            let sample = context.sample(FooRng, |rng| parameters.sample(rng));
            if sample == 0 {
                zero_counter += 1;
            }
        }
        // The expected value of `zero_counter` is 1000.
        assert!((zero_counter - 1000_i32).abs() < 100);
    }

    #[test]
    fn sample_distribution() {
        let mut context = Context::new();
        context.init_random(42);

        // Initialize weighted sampler. Zero is selected with probability 1/3, one with a
        // probability of 2/3.
        *context.get_data_mut(SamplerData) = WeightedIndex::new(vec![1.0, 2.0]).unwrap();

        let parameters = context.get_data(SamplerData);
        let n_samples = 3000;
        let mut zero_counter = 0;
        for _ in 0..n_samples {
            let sample = context.sample_distr(FooRng, parameters);
            if sample == 0 {
                zero_counter += 1;
            }
        }
        // The expected value of `zero_counter` is 1000.
        assert!((zero_counter - 1000_i32).abs() < 100);
    }

    #[test]
    fn sample_range() {
        let mut context = Context::new();
        context.init_random(42);
        let result = context.sample_range(FooRng, 0..10);
        assert!((0..10).contains(&result));
    }

    #[test]
    fn sample_bool() {
        let mut context = Context::new();
        context.init_random(42);
        let _r: bool = context.sample_bool(FooRng, 0.5);
    }

    #[test]
    fn sample_weighted() {
        let mut context = Context::new();
        context.init_random(42);
        let r: usize = context.sample_weighted(FooRng, &[0.1, 0.3, 0.4]);
        assert!(r < 3);
    }
}
