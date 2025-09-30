//! Algorithms for uniform random sampling from hash sets or iterators. These algorithms are written to be generic
//! over the container type using zero-cost trait abstractions.
use crate::rand::{seq::index::sample as choose_range, Rng};

/// Sample a random element uniformly from an iterator of known length.
///
/// We do not assume the set is randomly indexable, only that it can be iterated over. The value is cloned.
/// This algorithm is used when the property is indexed, and thus we know the length of the result set.
pub fn sample_single_from_known_length<I, R, T>(rng: &mut R, iter: &mut I, len: usize) -> Option<T>
where
    R: Rng,
    I: Iterator<Item = T>,
    T: Clone + 'static,
{
    if len == 0 {
        return None;
    }
    // This little trick with `u32` makes this function 30% faster.
    let index = rng.gen_range(0..len as u32) as usize;
    // The set need not be randomly indexable, so we have to use the `nth` method.
    iter.nth(index).clone()
}

/// Sample a random element uniformly from an iterator of unknown length.
///
/// We do not assume the set is randomly indexable, only that it can be iterated over. The value is cloned.
///
/// This function implements "Algorithm L" from KIM-HUNG LI
/// Reservoir-Sampling Algorithms of Time Complexity O(n(1 + log(N/n)))
/// <https://dl.acm.org/doi/pdf/10.1145/198429.198435>
///
/// This algorithm is significantly slower than the "known length" algorithm (factor
/// of 10^4). The reservoir algorithm from `rand` reduces to the "known length`
/// algorithm when the iterator is an `ExactSizeIterator`, or more precisely,
/// when `iterator.size_hint()` returns `(k, Some(k))` for some `k`. Otherwise,
/// this algorithm is much faster than the `rand` implementation (factor of 100).
pub fn sample_single_l_reservoir<I, R, T>(rng: &mut R, iter: &mut I) -> Option<T>
where
    R: Rng,
    I: Iterator<Item = T>,
    T: Clone + 'static,
{
    let mut chosen_item: Option<T> = None; // the currently selected element
    let mut weight: f64 = rng.gen_range(0.0..1.0); // controls skip distance distribution
    let mut position: usize = 0; // current index in data
    let mut next_pick_position: usize = 1; // index of the next item to pick

    iter.for_each(|item| {
        position += 1;
        if position == next_pick_position {
            chosen_item = Some(item.clone());
            // `f32` arithmetic is no faster than `f64` on modern hardware.
            next_pick_position +=
                (f64::ln(rng.gen_range(0.0..1.0)) / f64::ln(1.0 - weight)).floor() as usize + 1;
            weight *= rng.gen_range(0.0..1.0);
        }
    });

    chosen_item
}

/// Sample multiple random elements uniformly without replacement from a set of known length.
/// This function assumes `set.len() >= requested`.
///
/// We do not assume the set is randomly indexable, only that it can be iterated over. The values are cloned.
///
/// This algorithm can be used when the property is indexed, and thus we know the length of the result set.
/// For very small `requested` values (<=5), this algorithm is faster than reservoir because it doesn't
/// iterate over the entire set.
pub fn sample_multiple_from_known_length<I, R, T>(
    rng: &mut R,
    iter: &mut I,
    len: usize,
    requested: usize,
) -> Vec<T>
where
    R: Rng,
    I: Iterator<Item = T>,
    T: Clone + 'static,
{
    let mut indexes = Vec::with_capacity(requested);
    indexes.extend(choose_range(rng, len, requested));
    indexes.sort_unstable();
    let mut index_iterator = indexes.into_iter();
    let mut next_idx = index_iterator.next().unwrap();
    let mut selected = Vec::with_capacity(requested);

    for (idx, item) in iter.enumerate() {
        if idx == next_idx {
            selected.push(item.clone());
            if let Some(i) = index_iterator.next() {
                next_idx = i;
            } else {
                break;
            }
        }
    }

    selected
}

/// Sample multiple random elements uniformly without replacement from a set of unknown length. If
/// more samples are requested than are in the set, the function returns as many items as it can.
///
/// We do not assume the set is randomly indexable, only that it can be iterated over. The values are cloned.
///
/// This function implements "Algorithm L" from KIM-HUNG LI
/// Reservoir-Sampling Algorithms of Time Complexity O(n(1 + log(N/n)))
/// <https://dl.acm.org/doi/pdf/10.1145/198429.198435>
///
/// This algorithm is significantly faster than the reservoir algorithm in `rand` and is
/// on par with the "known length" algorithm for large `requested` values.
pub fn sample_multiple_l_reservoir<I, R, T>(rng: &mut R, iter: &mut I, requested: usize) -> Vec<T>
where
    R: Rng,
    I: Iterator<Item = T>,
    T: Clone + 'static,
{
    let mut weight: f64 = rng.gen_range(0.0..1.0); // controls skip distance distribution
    let mut position: usize = 0; // current index in data
    let mut next_pick_position: usize = 1; // index of the next item to pick
    let mut reservoir = Vec::with_capacity(requested); // the sample reservoir

    iter.for_each(|item| {
        position += 1;
        if position == next_pick_position {
            if reservoir.len() == requested {
                let to_remove = rng.gen_range(0..reservoir.len());
                reservoir.swap_remove(to_remove);
            }
            reservoir.push(item.clone());

            if reservoir.len() == requested {
                next_pick_position +=
                    (f64::ln(rng.gen_range(0.0..1.0)) / f64::ln(1.0 - weight)).floor() as usize + 1;
                weight *= rng.gen_range(0.0..1.0);
            } else {
                next_pick_position += 1;
            }
        }
    });

    reservoir
}
