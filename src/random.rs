use crate::context::Context;
use rand::distributions::uniform::SampleRange;
use rand::distributions::uniform::SampleUniform;
use rand::prelude::Distribution;
use rand::{Rng, SeedableRng};
use std::any::{Any, TypeId};
use std::cell::{RefCell, RefMut};
use std::collections::HashMap;

/// Use this to define a unique type which will be used as a key to retrieve
/// an independent rng instance when calling `.get_rng`.
#[macro_export]
macro_rules! define_rng {
    ($random_id:ident) => {
        struct $random_id;

        impl $crate::random::RngId for $random_id {
            // TODO(ryl8@cdc.gov): This is hardcoded to StdRng; we should replace this
            type RngType = rand::rngs::StdRng;

            fn get_name() -> &'static str {
                stringify!($random_id)
            }
        }

        // This ensures that you can't define two RngIds with the same name
        paste::paste! {
            #[doc(hidden)]
            #[no_mangle]
            pub static [<rng_name_duplication_guard_ $random_id>]: () = ();
        }
    };
}
pub use define_rng;

pub trait RngId {
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
        .get_data_container::<RngPlugin>()
        .expect("You must initialize the random number generator with a base seed");

    let rng_holders = data_container.rng_holders.try_borrow_mut().unwrap();
    RefMut::map(rng_holders, |holders| {
        holders
            .entry(TypeId::of::<R>())
            // Create a new rng holder if it doesn't exist yet
            .or_insert_with(|| {
                let base_seed = data_container.base_seed;
                let seed_offset = fxhash::hash64(R::get_name());
                RngHolder {
                    rng: Box::new(R::RngType::seed_from_u64(base_seed + seed_offset)),
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

    fn sample_range<R: RngId + 'static, S, T>(&self, rng_type: R, range: S) -> T
    where
        R::RngType: Rng,
        S: SampleRange<T>,
        T: SampleUniform;

    fn sample_bool<R: RngId + 'static>(&self, rng_id: R, p: f64) -> bool
    where
        R::RngType: Rng;
}

impl ContextRandomExt for Context {
    /// Initializes the `RngPlugin` data container to store rngs as well as a base
    /// seed. Note that rngs are created lazily when `get_rng` is called.
    fn init_random(&mut self, base_seed: u64) {
        let data_container = self.get_data_container_mut::<RngPlugin>();
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
            context.sample(FooRng, |rng| rng.next_u64()),
            context.sample(FooRng, |rng| rng.next_u64())
        );
    }

    #[test]
    #[should_panic(expected = "You must initialize the random number generator with a base seed")]
    fn panic_if_not_initialized() {
        let context = Context::new();
        context.sample(FooRng, |rng| rng.next_u64());
    }

    #[test]
    fn multiple_rng_types() {
        let mut context = Context::new();
        context.init_random(42);

        assert_ne!(
            context.sample(FooRng, |rng| rng.next_u64()),
            context.sample(BarRng, |rng| rng.next_u64())
        );
    }

    #[test]
    fn reset_seed() {
        let mut context = Context::new();
        context.init_random(42);

        let run_0 = context.sample(FooRng, |rng| rng.next_u64());
        let run_1 = context.sample(FooRng, |rng| rng.next_u64());

        // Reset with same seed, ensure we get the same values
        context.init_random(42);
        assert_eq!(run_0, context.sample(FooRng, |rng| rng.next_u64()));
        assert_eq!(run_1, context.sample(FooRng, |rng| rng.next_u64()));

        // Reset with different seed, ensure we get different values
        context.init_random(88);
        assert_ne!(run_0, context.sample(FooRng, |rng| rng.next_u64()));
        assert_ne!(run_1, context.sample(FooRng, |rng| rng.next_u64()));
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
        // Initialize weighted sampler
        *context.get_data_container_mut::<SamplerData>() =
            WeightedIndex::new(vec![1.0, 2.0]).unwrap();

        let parameters = context.get_data_container::<SamplerData>().unwrap();
        let n_samples = 3000;
        let mut zero_counter = 0;
        for _ in 0..n_samples {
            let sample = context.sample(FooRng, |rng| parameters.sample(rng));
            if sample == 0 {
                zero_counter += 1;
            }
        }
        assert!((zero_counter - 1000 as i32).abs() < 30);
    }

    #[test]
    fn sample_distribution() {
        let mut context = Context::new();
        context.init_random(42);

        // Initialize weighted sampler
        *context.get_data_container_mut::<SamplerData>() =
            WeightedIndex::new(vec![1.0, 2.0]).unwrap();

        let parameters = context.get_data_container::<SamplerData>().unwrap();
        let n_samples = 3000;
        let mut zero_counter = 0;
        for _ in 0..n_samples {
            let sample = context.sample_distr(FooRng, parameters);
            if sample == 0 {
                zero_counter += 1;
            }
        }
        assert!((zero_counter - 1000 as i32).abs() < 30);
    }

    #[test]
    fn sample_range() {
        let mut context = Context::new();
        context.init_random(42);
        let result = context.sample_range(FooRng, 0..10);
        assert!(result >= 0 && result < 10);
    }

    #[test]
    fn sample_bool() {
        let mut context = Context::new();
        context.init_random(42);
        let result = context.sample_bool(FooRng, 0.5);
        assert!(result == true || result == false);
    }
}
