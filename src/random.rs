use crate::context::Context;
use crate::hashing::hash_str;
use crate::{HashMap, HashMapExt};
use log::trace;
use rand::distributions::uniform::{SampleRange, SampleUniform};
use rand::distributions::WeightedIndex;
use rand::prelude::Distribution;
use rand::{Rng, SeedableRng};
use std::any::{Any, TypeId};
use std::cell::{RefCell, RefMut};

/// Use this to define a unique type which will be used as a key to retrieve
/// an independent rng instance when calling `.get_rng`.
#[macro_export]
macro_rules! define_rng {
    ($random_id:ident) => {
        #[derive(Copy, Clone)]
        struct $random_id;

        impl $crate::random::RngId for $random_id {
            type RngType = $crate::rand::rngs::SmallRng;

            fn get_name() -> &'static str {
                stringify!($random_id)
            }
        }

        // This ensures that you can't define two RngIds with the same name
        $crate::paste::paste! {
            #[doc(hidden)]
            #[no_mangle]
            #[allow(non_upper_case_globals)]
            pub static [<rng_name_duplication_guard_ $random_id>]: () = ();
        }
    };
}
pub use define_rng;

pub trait RngId: Copy + Clone {
    type RngType: SeedableRng;
    fn get_name() -> &'static str;
}

// This is a wrapper which allows for future support for different types of
// random number generators (anything that implements SeedableRng is valid).
struct RngHolder {
    rng: Box<dyn Any>,
}

struct RngData {
    base_seed: u64,
    rng_holders: RefCell<HashMap<TypeId, RngHolder>>,
}

// Registers a data container which stores:
// * base_seed: A base seed for all rngs
// * rng_holders: A map of rngs, keyed by their RngId. Note that this is
//   stored in a RefCell to allow for mutable borrow without requiring a
//   mutable borrow of the Context itself.
crate::context::define_data_plugin!(
    RngPlugin,
    RngData,
    RngData {
        base_seed: 0,
        rng_holders: RefCell::new(HashMap::new()),
    }
);

/// Gets a mutable reference to the random number generator associated with the given
/// `RngId`. If the Rng has not been used before, one will be created with the base seed
/// you defined in `init`. Note that this will panic if `init` was not called yet.
fn get_rng<R: RngId + 'static>(context: &Context) -> RefMut<R::RngType> {
    let data_container = context
        .get_data_container(RngPlugin)
        .expect("You must initialize the random number generator with a base seed");

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

// This is a trait exension on Context
pub trait ContextRandomExt {
    fn init_random(&mut self, base_seed: u64);

    /// Gets a random sample from the random number generator associated with the given
    /// `RngId` by applying the specified sampler function. If the Rng has not been used
    /// before, one will be created with the base seed you defined in `set_base_random_seed`.
    /// Note that this will panic if `set_base_random_seed` was not called yet.
    fn sample<R: RngId + 'static, T>(
        &self,
        _rng_type: R,
        sampler: impl FnOnce(&mut R::RngType) -> T,
    ) -> T;

    /// Gets a random sample from the specified distribution using a random number generator
    /// associated with the given `RngId`. If the Rng has not been used before, one will be
    /// created with the base seed you defined in `set_base_random_seed`.
    /// Note that this will panic if `set_base_random_seed` was not called yet.
    fn sample_distr<R: RngId + 'static, T>(
        &self,
        _rng_type: R,
        distribution: impl Distribution<T>,
    ) -> T
    where
        R::RngType: Rng;

    /// Gets a random sample within the range provided by `range`
    /// using the generator associated with the given `RngId`.
    /// Note that this will panic if `set_base_random_seed` was not called yet.
    fn sample_range<R: RngId + 'static, S, T>(&self, rng_type: R, range: S) -> T
    where
        R::RngType: Rng,
        S: SampleRange<T>,
        T: SampleUniform;

    /// Gets a random boolean value which is true with probability `p`
    /// using the generator associated with the given `RngId`.
    /// Note that this will panic if `set_base_random_seed` was not called yet.
    fn sample_bool<R: RngId + 'static>(&self, rng_id: R, p: f64) -> bool
    where
        R::RngType: Rng;

    /// Draws a random entry out of the list provided in `weights`
    /// with the given weights using the generator associated with the
    /// given `RngId`.  Note that this will panic if
    /// `set_base_random_seed` was not called yet.
    fn sample_weighted<R: RngId + 'static, T>(&self, rng_id: R, weights: &[T]) -> usize
    where
        R::RngType: Rng,
        T: Clone + Default + SampleUniform + for<'a> std::ops::AddAssign<&'a T> + PartialOrd;
}

impl ContextRandomExt for Context {
    /// Initializes the `RngPlugin` data container to store rngs as well as a base
    /// seed. Note that rngs are created lazily when `get_rng` is called.
    fn init_random(&mut self, base_seed: u64) {
        trace!("initializing random module");
        let data_container = self.get_data_container_mut(RngPlugin);
        data_container.base_seed = base_seed;

        // Clear any existing Rngs to ensure they get re-seeded when `get_rng` is called
        let mut rng_map = data_container.rng_holders.try_borrow_mut().unwrap();
        rng_map.clear();
    }

    fn sample<R: RngId + 'static, T>(
        &self,
        _rng_id: R,
        sampler: impl FnOnce(&mut R::RngType) -> T,
    ) -> T {
        let mut rng = get_rng::<R>(self);
        sampler(&mut rng)
    }

    fn sample_distr<R: RngId + 'static, T>(
        &self,
        _rng_id: R,
        distribution: impl Distribution<T>,
    ) -> T
    where
        R::RngType: Rng,
    {
        let mut rng = get_rng::<R>(self);
        distribution.sample::<R::RngType>(&mut rng)
    }

    fn sample_range<R: RngId + 'static, S, T>(&self, rng_id: R, range: S) -> T
    where
        R::RngType: Rng,
        S: SampleRange<T>,
        T: SampleUniform,
    {
        self.sample(rng_id, |rng| rng.gen_range(range))
    }

    fn sample_bool<R: RngId + 'static>(&self, rng_id: R, p: f64) -> bool
    where
        R::RngType: Rng,
    {
        self.sample(rng_id, |rng| rng.gen_bool(p))
    }

    fn sample_weighted<R: RngId + 'static, T>(&self, _rng_id: R, weights: &[T]) -> usize
    where
        R::RngType: Rng,
        T: Clone + Default + SampleUniform + for<'a> std::ops::AddAssign<&'a T> + PartialOrd,
    {
        let index = WeightedIndex::new(weights).unwrap();
        let mut rng = get_rng::<R>(self);
        index.sample(&mut *rng)
    }
}

#[cfg(test)]
mod test {
    use crate::context::Context;
    use crate::define_data_plugin;
    use crate::random::ContextRandomExt;
    use rand::RngCore;
    use rand::{distributions::WeightedIndex, prelude::Distribution};

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
    #[should_panic(expected = "You must initialize the random number generator with a base seed")]
    fn panic_if_not_initialized() {
        let context = Context::new();
        context.sample(FooRng, RngCore::next_u64);
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
        *context.get_data_container_mut(SamplerData) = WeightedIndex::new(vec![1.0, 2.0]).unwrap();

        let parameters = context.get_data_container(SamplerData).unwrap();
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
        *context.get_data_container_mut(SamplerData) = WeightedIndex::new(vec![1.0, 2.0]).unwrap();

        let parameters = context.get_data_container(SamplerData).unwrap();
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
