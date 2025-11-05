//! Algorithms for uniform random sampling from hash sets or iterators. These algorithms are written to be generic
//! over the container type using zero-cost trait abstractions.
use std::collections::{HashMap, HashSet};

use crate::rand::seq::index::sample as choose_range;
use crate::rand::Rng;

/// The `len` capability, a zero-cost abstraction for types that have a known length.
pub trait HasLen {
    fn len(&self) -> usize;
}

/// The `iter` capability, a zero-cost abstraction for types that can be iterated over.
pub trait HasIter {
    type Item<'a>
    where
        Self: 'a;
    type Iter<'a>: Iterator<Item = Self::Item<'a>>
    where
        Self: 'a;

    fn iter(&self) -> Self::Iter<'_>;
}

macro_rules! impl_has_len {
    ($ty:ident < $($gen:ident),* >) => {
        impl<$($gen),*> HasLen for $ty<$($gen),*> {
            fn len(&self) -> usize {
                <$ty<$($gen),*>>::len(self)
            }
        }
    };
}

macro_rules! impl_has_iter {
    ($ty:ident < $($gen:ident),* >, $iter:ty, $item:ty) => {
        impl<$($gen),*> HasIter for $ty<$($gen),*> {
            type Item<'a> = $item where Self: 'a;
            type Iter<'a> = $iter where Self: 'a;

            fn iter(&self) -> Self::Iter<'_> {
                <$ty<$($gen),*>>::iter(self)
            }
        }
    };
}

// Vec<T>
impl_has_len!(Vec<T>);
// We implement `HasIter` manually for `Vec<T>` because its `iter` method is from `Deref<Target = [T]>`.
impl<T> HasIter for Vec<T> {
    type Item<'a>
        = &'a T
    where
        Self: 'a;
    type Iter<'a>
        = std::slice::Iter<'a, T>
    where
        Self: 'a;

    fn iter(&self) -> Self::Iter<'_> {
        <[T]>::iter(self)
    }
}

// HashSet<T, H>
impl_has_len!(HashSet<T, H>);
impl_has_iter!(HashSet<T, H>, std::collections::hash_set::Iter<'a, T>, &'a T);

// HashMap<K, V, H>
impl_has_len!(HashMap<K, V, H>);
impl_has_iter!(HashMap<K, V, H>, std::collections::hash_map::Iter<'a, K, V>, (&'a K, &'a V));

/// Sample a random element uniformly from a container of known length.
///
/// We do not assume the container is randomly indexable, only that it can be iterated over. The value is cloned.
/// This algorithm is used when the property is indexed, and thus we know the length of the result set.
pub fn sample_single_from_known_length<'a, Container, R, T>(
    rng: &mut R,
    set: &'a Container,
) -> Option<T>
where
    R: Rng,
    Container: HasLen + HasIter<Item<'a> = &'a T>,
    T: Clone + 'static,
{
    let len = set.len();
    if len == 0 {
        return None;
    }
    // This little trick with `u32` makes this function 30% faster.
    let index = rng.random_range(0..len as u32) as usize;
    // The set need not be randomly indexable, so we have to use the `nth` method.
    set.iter().nth(index).cloned()
}

/// Sample a random element uniformly from a container of unknown length.
///
/// We do not assume the container is randomly indexable, only that it can be iterated over. The value is cloned.
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
// ToDo(RobertJacobsonCDC): This function will take an iterator once the `iter_query_results` API is ready.
pub fn sample_single_l_reservoir<'a, Container, R, T>(rng: &mut R, set: &'a Container) -> Option<T>
where
    R: Rng,
    Container: HasIter<Item<'a> = &'a T>,
    T: Clone + 'static,
{
    let mut chosen_item: Option<T> = None; // the currently selected element
    let mut weight: f64 = rng.random_range(0.0..1.0); // controls skip distance distribution
    let mut position: usize = 0; // current index in data
    let mut next_pick_position: usize = 1; // index of the next item to pick

    set.iter().for_each(|item| {
        position += 1;
        if position == next_pick_position {
            chosen_item = Some(item.clone());
            next_pick_position +=
                (f64::ln(rng.random_range(0.0..1.0)) / f64::ln(1.0 - weight)).floor() as usize + 1;
            weight *= rng.random_range(0.0..1.0);
        }
    });

    chosen_item
}

/// Sample multiple random elements uniformly without replacement from a container of known length.
/// This function assumes `set.len() >= requested`.
///
/// We do not assume the container is randomly indexable, only that it can be iterated over. The values are cloned.
///
/// This algorithm can be used when the property is indexed, and thus we know the length of the result set.
/// For very small `requested` values (<=5), this algorithm is faster than reservoir because it doesn't
/// iterate over the entire set.
pub fn sample_multiple_from_known_length<'a, Container, R, T>(
    rng: &mut R,
    set: &'a Container,
    requested: usize,
) -> Vec<T>
where
    R: Rng,
    Container: HasLen + HasIter<Item<'a> = &'a T>,
    T: Clone + 'static,
{
    let mut indexes = Vec::with_capacity(requested);
    indexes.extend(choose_range(rng, set.len(), requested));
    indexes.sort_unstable();
    let mut index_iterator = indexes.into_iter();
    let mut next_idx = index_iterator.next().unwrap();
    let mut selected = Vec::with_capacity(requested);

    for (idx, item) in set.iter().enumerate() {
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

/// Sample multiple random elements uniformly without replacement from a container of known length. If
/// more samples are requested than are in the set, the function returns as many items as it can.
///
/// We do not assume the container is randomly indexable, only that it can be iterated over. The values are cloned.
///
/// This function implements "Algorithm L" from KIM-HUNG LI
/// Reservoir-Sampling Algorithms of Time Complexity O(n(1 + log(N/n)))
/// <https://dl.acm.org/doi/pdf/10.1145/198429.198435>
///
/// This algorithm is significantly faster than the reservoir algorithm in `rand` and is
/// on par with the "known length" algorithm for large `requested` values.
// ToDo(RobertJacobsonCDC): This function will take an iterator once the `iter_query_results` API is ready.
pub fn sample_multiple_l_reservoir<'a, Container, R, T>(
    rng: &mut R,
    set: &'a Container,
    requested: usize,
) -> Vec<T>
where
    R: Rng,
    Container: HasLen + HasIter<Item<'a> = &'a T>,
    T: Clone + 'static,
{
    let mut weight: f64 = rng.random_range(0.0..1.0); // controls skip distance distribution
    let mut position: usize = 0; // current index in data
    let mut next_pick_position: usize = 1; // index of the next item to pick
    let mut reservoir = Vec::with_capacity(requested); // the sample reservoir

    set.iter().for_each(|item| {
        position += 1;
        if position == next_pick_position {
            if reservoir.len() == requested {
                let to_remove = rng.random_range(0..reservoir.len());
                reservoir.swap_remove(to_remove);
            }
            reservoir.push(item.clone());

            if reservoir.len() == requested {
                next_pick_position += (f64::ln(rng.random_range(0.0..1.0)) / f64::ln(1.0 - weight))
                    .floor() as usize
                    + 1;
                weight *= rng.random_range(0.0..1.0);
            } else {
                next_pick_position += 1;
            }
        }
    });

    reservoir
}
