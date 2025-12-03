mod context_ext;
mod macros;
mod sampling_algorithms;

use std::any::{Any, TypeId};
use std::cell::RefCell;

pub use context_ext::ContextRandomExt;
pub use macros::define_rng;
pub use sampling_algorithms::{
    sample_multiple_from_known_length, sample_multiple_l_reservoir,
    sample_single_from_known_length, sample_single_l_reservoir,
};

use crate::rand::SeedableRng;
use crate::{define_data_plugin, HashMap, HashMapExt};

pub trait RngId: Copy + Clone {
    type RngType: SeedableRng;
    fn get_name() -> &'static str;
}

// This is a wrapper that allows for future support for different types of
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
define_data_plugin!(
    RngPlugin,
    RngData,
    RngData {
        base_seed: 0,
        rng_holders: RefCell::new(HashMap::new()),
    }
);
