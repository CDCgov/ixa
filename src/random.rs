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
            // TODO: This is hardcoded to StdRng; we should replace this
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

crate::context::define_data_plugin!(
    RngPlugin,
    RngData,
    RngData {
        base_seed: 0,
        rng_holders: RefCell::new(HashMap::new()),
    }
);

#[allow(clippy::module_name_repetitions)]
pub trait RandomContext {
    fn set_base_random_seed(&mut self, base_seed: u64);

    fn get_rng<R: RngId>(&self) -> RefMut<'_, R::RngType>;
}

impl RandomContext for Context {
    /// Initializes the `RngPlugin` data container to store rngs as well as a base
    /// seed. Note that rngs are created lazily when `get_rng` is called.
    fn set_base_random_seed(&mut self, base_seed: u64) {
        let data_container = self.get_data_container_mut::<RngPlugin>();
        data_container.base_seed = base_seed;

        // Clear any existing Rngs to ensure they get re-seeded when `get_rng` is called
        let mut rng_map = data_container.rng_holders.try_borrow_mut().unwrap();
        rng_map.clear();
    }

    /// Gets a mutable reference to the random number generator associated with the given
    /// `RngId`. If the Rng has not been used before, one will be created with the base seed
    /// you defined in `set_base_random_seed`. Note that this will panic if `set_base_random_seed` was not called yet.
    fn get_rng<R: RngId>(&self) -> RefMut<'_, R::RngType> {
        let data_container = self
            .get_data_container::<RngPlugin>()
            .expect("You must initialize the random number generator with a base seed");
        let random_holders = data_container.rng_holders.try_borrow_mut().unwrap();

        let random_holder = RefMut::map(random_holders, |random_holders| {
            random_holders.entry(TypeId::of::<R>()).or_insert_with(|| {
                let base_seed = data_container.base_seed;
                let seed_offset = fxhash::hash64(R::get_name());
                RngHolder {
                    rng: Box::new(R::RngType::seed_from_u64(base_seed + seed_offset)),
                }
            })
        });

        RefMut::map(random_holder, |random_holder| {
            random_holder.rng.downcast_mut::<R::RngType>().unwrap()
        })
    }
}

#[cfg(test)]
mod test {
    use crate::context::Context;
    use crate::random::RandomContext;
    use rand::RngCore;

    define_rng!(FooRng);
    define_rng!(BarRng);

    #[test]
    fn get_rng_basic() {
        let mut context = Context::new();
        context.set_base_random_seed(42);

        let mut foo_rng = context.get_rng::<FooRng>();
        assert_eq!(foo_rng.next_u64(), 5113542052170610017);
        assert_eq!(foo_rng.next_u64(), 8640506012583485895);
        assert_eq!(foo_rng.next_u64(), 16699691489468094833);
    }

    #[test]
    #[should_panic(expected = "You must initialize the random number generator with a base seed")]
    fn panic_if_not_initialized() {
        let context = Context::new();
        context.get_rng::<FooRng>();
    }

    #[test]
    #[should_panic]
    fn get_rng_one_ref_per_rng_id() {
        let mut context = Context::new();
        context.set_base_random_seed(42);
        let mut foo_rng = context.get_rng::<FooRng>();

        // This should panic because we already have a mutable reference to FooRng
        let mut foo_rng_2 = context.get_rng::<BarRng>();
        foo_rng.next_u64();
        foo_rng_2.next_u64();
    }

    #[test]
    fn get_rng_two_types() {
        let mut context = Context::new();
        context.set_base_random_seed(42);

        let mut foo_rng = context.get_rng::<FooRng>();
        foo_rng.next_u64();
        drop(foo_rng);

        let mut bar_rng = context.get_rng::<BarRng>();
        bar_rng.next_u64();
    }

    #[test]
    fn reset_seed() {
        let mut context = Context::new();
        context.set_base_random_seed(42);

        let mut foo_rng = context.get_rng::<FooRng>();
        let run_0 = foo_rng.next_u64();
        let run_1 = foo_rng.next_u64();
        drop(foo_rng);

        // Reset with same seed, ensure we get the same values
        context.set_base_random_seed(42);
        let mut foo_rng = context.get_rng::<FooRng>();
        assert_eq!(run_0, foo_rng.next_u64());
        assert_eq!(run_1, foo_rng.next_u64());
        drop(foo_rng);

        // Reset with different seed, ensure we get different values
        context.set_base_random_seed(88);
        let mut foo_rng = context.get_rng::<FooRng>();
        assert_ne!(run_0, foo_rng.next_u64());
        assert_ne!(run_1, foo_rng.next_u64());
    }
}
