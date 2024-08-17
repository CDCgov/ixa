use crate::context::Context;
use rand::SeedableRng;
use std::any::{Any, TypeId};
use std::cell::{RefCell, RefMut};
use std::collections::HashMap;

/// Use this to define a unique type which will be used as a key to retrieve
/// an independent rng instance when calling `.get_rng`.
#[macro_export]
macro_rules! define_rng {
    ($random_id:ident) => {
        struct $random_id {}

        impl $crate::random::RngId for $random_id {
            // TODO(ryl8@cdc.gov): This is hardcoded to StdRng; we should replace this
            type RngType = rand::rngs::StdRng;

            fn get_name() -> &'static str {
                stringify!($random_id)
            }
        }
    };
}
pub use define_rng;

pub trait RngId: Any {
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

// This is a trait exension on Context
pub trait ContextRandomExt {
    fn init_random(&mut self, base_seed: u64);

    fn get_rng<R: RngId>(&self) -> RefMut<R::RngType>;
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

    /// Gets a mutable reference to the random number generator associated with the given
    /// `RngId`. If the Rng has not been used before, one will be created with the base seed
    /// you defined in `init`. Note that this will panic if `init` was not called yet.
    fn get_rng<R: RngId + 'static>(&self) -> RefMut<R::RngType> {
        let data_container = self
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
}

#[cfg(test)]
mod test {
    use crate::context::Context;
    use crate::random::ContextRandomExt;
    use rand::RngCore;
    use rand_distr::{Distribution, Exp};

    define_rng!(FooRng);
    define_rng!(BarRng);

    #[test]
    fn get_rng_basic() {
        let mut context = Context::new();
        context.init_random(42);

        let mut foo_rng = context.get_rng::<FooRng>();

        assert_ne!(foo_rng.next_u64(), foo_rng.next_u64());
    }

    #[test]
    #[should_panic(expected = "You must initialize the random number generator with a base seed")]
    fn panic_if_not_initialized() {
        let context = Context::new();
        context.get_rng::<FooRng>();
    }

    #[test]
    #[should_panic]
    fn no_multiple_references_to_rngs() {
        let mut context = Context::new();
        context.init_random(42);
        let mut foo_rng = context.get_rng::<FooRng>();

        // This should panic because we already have a mutable reference to FooRng
        let mut foo_rng_2 = context.get_rng::<BarRng>();
        foo_rng.next_u64();
        foo_rng_2.next_u64();
    }

    #[test]
    fn multiple_references_with_drop() {
        let mut context = Context::new();
        context.init_random(42);

        let mut foo_rng = context.get_rng::<FooRng>();
        foo_rng.next_u64();
        // If you drop the first reference, you should be able to get a reference to a different rng
        drop(foo_rng);

        let mut bar_rng = context.get_rng::<BarRng>();
        bar_rng.next_u64();
    }

    #[test]
    fn usage_with_distribution() {
        let mut context = Context::new();
        context.init_random(42);
        let mut rng = context.get_rng::<FooRng>();
        let dist = Exp::new(1.0).unwrap();
        assert_ne!(dist.sample(&mut *rng), dist.sample(&mut *rng));
    }

    #[test]
    fn reset_seed() {
        let mut context = Context::new();
        context.init_random(42);

        let mut foo_rng = context.get_rng::<FooRng>();
        let run_0 = foo_rng.next_u64();
        let run_1 = foo_rng.next_u64();
        drop(foo_rng);

        // Reset with same seed, ensure we get the same values
        context.init_random(42);
        let mut foo_rng = context.get_rng::<FooRng>();
        assert_eq!(run_0, foo_rng.next_u64());
        assert_eq!(run_1, foo_rng.next_u64());
        drop(foo_rng);

        // Reset with different seed, ensure we get different values
        context.init_random(88);
        let mut foo_rng = context.get_rng::<FooRng>();
        assert_ne!(run_0, foo_rng.next_u64());
        assert_ne!(run_1, foo_rng.next_u64());
    }
}
