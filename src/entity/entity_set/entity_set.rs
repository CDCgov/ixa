//! A lazy, composable set type built from set-algebraic expressions.
//!
//! An [`EntitySet`] represents a set of [`EntityId`] values as a tree of union,
//! intersection, and difference operations over leaf [`SourceSet`] nodes. The tree
//! is constructed eagerly but evaluated lazily: membership tests ([`contains`]) walk
//! the tree on demand, and iteration is deferred to [`EntitySetIterator`].
//!
//! Construction methods reorder operands by estimated size to improve
//! short-circuit performance and apply only minimal structural simplification.
//!
//! [`contains`]: EntitySet::contains

use std::borrow::Borrow;
use std::ops::Range;

use log::warn;
use rand::Rng;

use super::{EntitySetIterator, SourceSet};
use crate::entity::{Entity, EntityId};
use crate::random::{
    count_and_sample_single_l_reservoir, sample_multiple_from_known_length,
    sample_multiple_l_reservoir, sample_single_excluding_l_reservoir, sample_single_l_reservoir,
};

/// Opaque public wrapper around the internal set-expression tree.
pub struct EntitySet<'a, E: Entity> {
    pub(super) inner: EntitySetInner<'a, E>,
    #[cfg(feature = "profiling")]
    pub(in crate::entity) query_profile: Option<crate::profiling::QueryProfileHandle<'a>>,
}

/// Internal set-expression tree used to represent composed query sources.
pub(super) enum EntitySetInner<'a, E: Entity> {
    Source(SourceSet<'a, E>),
    Union(Box<EntitySet<'a, E>>, Box<EntitySet<'a, E>>),
    Intersection(Vec<EntitySet<'a, E>>),
    Difference(Box<EntitySet<'a, E>>, Box<EntitySet<'a, E>>),
}

impl<'a, E: Entity> Clone for EntitySet<'a, E> {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
            #[cfg(feature = "profiling")]
            query_profile: self.query_profile,
        }
    }
}

impl<'a, E: Entity> Clone for EntitySetInner<'a, E> {
    fn clone(&self) -> Self {
        match self {
            Self::Source(source) => Self::Source(source.clone()),
            Self::Union(left, right) => Self::Union(left.clone(), right.clone()),
            Self::Intersection(sets) => Self::Intersection(sets.clone()),
            Self::Difference(left, right) => Self::Difference(left.clone(), right.clone()),
        }
    }
}

impl<'a, E: Entity> Default for EntitySet<'a, E> {
    fn default() -> Self {
        Self::empty()
    }
}

impl<'a, E: Entity> EntitySet<'a, E> {
    fn new(inner: EntitySetInner<'a, E>) -> Self {
        Self {
            inner,
            #[cfg(feature = "profiling")]
            query_profile: None,
        }
    }

    /// Set algebra retains profiling only when all profiled operands have the
    /// same query identity. This avoids charging a composed operation to an
    /// arbitrary query while preserving a single identifiable query origin.
    #[cfg(feature = "profiling")]
    fn unique_query_profile(
        left: &Self,
        right: &Self,
    ) -> Option<crate::profiling::QueryProfileHandle<'a>> {
        match (left.query_profile, right.query_profile) {
            (Some(left_profile), Some(right_profile))
                if left_profile.same_query_as(right_profile) =>
            {
                Some(left_profile)
            }
            (Some(_), Some(_)) => None,
            (profile @ Some(_), None) | (None, profile @ Some(_)) => profile,
            (None, None) => None,
        }
    }

    pub(super) fn is_source_leaf(&self) -> bool {
        matches!(self.inner, EntitySetInner::Source(_))
    }

    pub(super) fn into_source_leaf(self) -> Option<SourceSet<'a, E>> {
        match self.inner {
            EntitySetInner::Source(source) => Some(source),
            _ => None,
        }
    }

    /// Create an empty entity set.
    #[must_use]
    pub fn empty() -> Self {
        Self::new(EntitySetInner::Source(SourceSet::empty()))
    }

    /// Create an entity set from a single source set.
    pub(crate) fn from_source(source: SourceSet<'a, E>) -> Self {
        Self::new(EntitySetInner::Source(source))
    }

    pub(crate) fn from_intersection_sources(mut sources: Vec<SourceSet<'a, E>>) -> Self {
        match sources.len() {
            0 => return Self::empty(),
            1 => return Self::from_source(sources.pop().unwrap()),
            _ => {}
        }

        // Keep intersections sorted smallest-to-largest so iterators can take the
        // first source as the driver and membership checks short-circuit quickly.
        sources.sort_unstable_by_key(SourceSet::sort_key);

        let sets = sources.into_iter().map(Self::from_source).collect();

        Self::new(EntitySetInner::Intersection(sets))
    }

    #[cfg(feature = "profiling")]
    #[inline]
    pub(in crate::entity) fn with_query_profile(
        mut self,
        profile: crate::profiling::QueryProfileHandle<'a>,
    ) -> Self {
        self.query_profile = Some(profile);
        self
    }

    #[must_use]
    pub fn union(self, other: Self) -> Self {
        let left = self;
        let right = other;
        #[cfg(feature = "profiling")]
        let (left, right, query_profile) = {
            let query_profile = Self::unique_query_profile(&left, &right);
            let mut left = left;
            let mut right = right;
            left.query_profile = None;
            right.query_profile = None;
            (left, right, query_profile)
        };

        let result = 'result: {
            // Idempotence: A ∪ A = A  (same structure over same sources)
            if left.structurally_eq(&right) {
                break 'result left;
            }

            // Adjacent or overlapping intervals
            if let (Some(a), Some(b)) = (left.as_range(), right.as_range()) {
                if a.start <= b.end && b.start <= a.end {
                    break 'result Self::from_source(SourceSet::population_range(
                        a.start.min(b.start)..a.end.max(b.end),
                    ));
                }
            }

            // Union with empty set is identity: A ∪ ∅ = ∅ ∪ A = A
            match (left.is_empty(), right.is_empty()) {
                (true, _) => break 'result right,
                (_, true) => break 'result left,
                _ => { /* pass */ }
            }

            // Larger set on LHS: more likely to short-circuit `||`.
            let (left, right) = if left.sort_key() >= right.sort_key() {
                (left, right)
            } else {
                (right, left)
            };
            break 'result Self::new(EntitySetInner::Union(Box::new(left), Box::new(right)));
        };
        #[cfg(feature = "profiling")]
        let result = {
            let mut result = result;
            result.query_profile = query_profile;
            result
        };
        result
    }

    #[must_use]
    pub fn intersection(self, other: Self) -> Self {
        let left = self;
        let right = other;
        #[cfg(feature = "profiling")]
        let (left, right, query_profile) = {
            let query_profile = Self::unique_query_profile(&left, &right);
            let mut left = left;
            let mut right = right;
            left.query_profile = None;
            right.query_profile = None;
            (left, right, query_profile)
        };

        let result = 'result: {
            // Idempotence: A ∩ A = A
            if left.structurally_eq(&right) {
                break 'result left;
            }

            // Intersection of overlapping intervals
            if let (Some(a), Some(b)) = (left.as_range(), right.as_range()) {
                let overlap = a.start.max(b.start)..a.end.min(b.end);
                break 'result if overlap.is_empty() {
                    Self::empty()
                } else {
                    Self::from_source(SourceSet::population_range(overlap))
                };
            }

            // Intersection an empty set is empty: A ∩ ∅ = ∅ ∩ A = ∅
            match (left.is_empty(), right.is_empty()) {
                (true, _) => break 'result left,
                (_, true) => break 'result right,
                _ => { /* pass */ }
            }

            let mut sets = match left {
                EntitySet {
                    inner: EntitySetInner::Intersection(sets),
                    ..
                } => sets,
                _ => vec![left],
            };

            sets.push(right);
            // Keep intersections sorted smallest-to-largest so iterators can take the
            // first source as the driver and membership checks short-circuit quickly.
            sets.sort_unstable_by_key(EntitySet::sort_key);
            break 'result Self::new(EntitySetInner::Intersection(sets));
        };
        #[cfg(feature = "profiling")]
        let result = {
            let mut result = result;
            result.query_profile = query_profile;
            result
        };
        result
    }

    #[must_use]
    pub fn difference(self, other: Self) -> Self {
        let left = self;
        let right = other;
        #[cfg(feature = "profiling")]
        let (left, right, query_profile) = {
            let query_profile = Self::unique_query_profile(&left, &right);
            let mut left = left;
            let mut right = right;
            left.query_profile = None;
            right.query_profile = None;
            (left, right, query_profile)
        };

        let result = 'result: {
            // Self-subtraction: A \ A = ∅
            if left.structurally_eq(&right) {
                break 'result Self::empty();
            }

            if let (Some(a), Some(b)) = (left.as_range(), right.as_range()) {
                let overlap = a.start.max(b.start)..a.end.min(b.end);
                // Disjoint ranges leave the left operand unchanged.
                if overlap.is_empty() {
                    break 'result Self::from_source(SourceSet::population_range(a));
                }
                // A covering subtraction removes the entire left range.
                if overlap.start == a.start && overlap.end == a.end {
                    break 'result Self::empty();
                }
                // Trimming the left edge still leaves one contiguous suffix.
                if overlap.start == a.start {
                    break 'result Self::from_source(SourceSet::population_range(
                        overlap.end..a.end,
                    ));
                }
                // Trimming the right edge still leaves one contiguous prefix.
                if overlap.end == a.end {
                    break 'result Self::from_source(SourceSet::population_range(
                        a.start..overlap.start,
                    ));
                }
                // An interior subtraction would split the range, so keep the generic difference node.
            }

            // Subtraction involving an empty set is identity: A \ ∅ = A, ∅ \ A = ∅
            if left.is_empty() || right.is_empty() {
                break 'result left;
            }

            break 'result Self::new(EntitySetInner::Difference(Box::new(left), Box::new(right)));
        };
        #[cfg(feature = "profiling")]
        let result = {
            let mut result = result;
            result.query_profile = query_profile;
            result
        };
        result
    }

    /// Test whether `entity_id` is a member of this set.
    #[must_use]
    pub fn contains(&self, entity_id: EntityId<E>) -> bool {
        #[cfg(feature = "profiling")]
        let _query_profile_scope = self
            .query_profile
            .map(crate::profiling::QueryProfileHandle::scope);
        self.contains_impl(entity_id)
    }

    fn contains_impl(&self, entity_id: EntityId<E>) -> bool {
        match &self.inner {
            EntitySetInner::Source(source) => source.contains(entity_id),
            EntitySetInner::Union(a, b) => a.contains_impl(entity_id) || b.contains_impl(entity_id),
            EntitySetInner::Intersection(sets) => {
                sets.iter().all(|set| set.contains_impl(entity_id))
            }
            EntitySetInner::Difference(a, b) => {
                a.contains_impl(entity_id) && !b.contains_impl(entity_id)
            }
        }
    }

    /// Collect this set's contents into an owned vector of `EntityId<E>`.
    #[must_use]
    pub fn to_owned_vec(self) -> Vec<EntityId<E>> {
        self.into_iter().collect()
    }

    /// Sample a single entity uniformly from this set, excluding any entity
    /// equal to `excluded`. Returns `None` if the set is empty or contains
    /// only the excluded entity.
    ///
    /// For source-leaf sets with random-access backing (`PopulationRange`,
    /// `IndexSet`), runs in O(1) with at most two index lookups and no
    /// iterator construction. Falls back to O(n) reservoir sampling for
    /// composite sets and `PropertySet` sources.
    #[must_use]
    pub fn sample_entity_excluding<R, X>(&self, rng: &mut R, excluded: X) -> Option<EntityId<E>>
    where
        R: Rng,
        X: Borrow<EntityId<E>>,
    {
        #[cfg(feature = "profiling")]
        let _query_profile_scope = self
            .query_profile
            .map(crate::profiling::QueryProfileHandle::scope);

        let excluded = *excluded.borrow();
        if let Some(n) = self.try_len_impl() {
            if n == 0 {
                None
            } else {
                let p = rng.random_range(0..n as u32) as usize;
                match self.try_nth(p) {
                    Some(candidate) if candidate != excluded => Some(candidate),
                    Some(_) if n == 1 => None,
                    Some(_) => {
                        // `excluded` is at position `p`. Resample from the n-1 remaining
                        // positions: pick `k` uniform in `[0, n-1)`, then map it around
                        // the hole at `p`.
                        let k = rng.random_range(0..(n - 1) as u32) as usize;
                        let target = if k < p { k } else { k + 1 };
                        self.try_nth(target)
                    }
                    None => None,
                }
            }
        } else {
            let set = self.clone();
            #[cfg(feature = "profiling")]
            let set = {
                let mut set = set;
                set.query_profile = None;
                set
            };
            sample_single_excluding_l_reservoir(rng, set, excluded)
        }
    }

    /// Sample a single entity uniformly from this set. Returns `None` if the
    /// set is empty.
    #[must_use]
    pub fn sample_entity<R>(&self, rng: &mut R) -> Option<EntityId<E>>
    where
        R: Rng,
    {
        #[cfg(feature = "profiling")]
        let _query_profile_scope = self
            .query_profile
            .map(crate::profiling::QueryProfileHandle::scope);

        if let Some(n) = self.try_len_impl() {
            if n == 0 {
                warn!("Requested a sample entity from an empty population");
                return None;
            }
            // The `u32` cast makes this function 30% faster than `usize`.
            let index = rng.random_range(0..n as u32) as usize;
            return self.try_nth(index);
        }
        let set = self.clone();
        #[cfg(feature = "profiling")]
        let set = {
            let mut set = set;
            set.query_profile = None;
            set
        };
        sample_single_l_reservoir(rng, set)
    }

    /// Count the entities in this set and sample one uniformly from them.
    ///
    /// Returns `(count, sample)` where `sample` is `None` iff `count == 0`.
    #[must_use]
    pub fn count_and_sample_entity<R>(&self, rng: &mut R) -> (usize, Option<EntityId<E>>)
    where
        R: Rng,
    {
        #[cfg(feature = "profiling")]
        let _query_profile_scope = self
            .query_profile
            .map(crate::profiling::QueryProfileHandle::scope);

        if let Some(n) = self.try_len_impl() {
            if n == 0 {
                return (0, None);
            }
            let index = rng.random_range(0..n as u32) as usize;
            return (n, self.try_nth(index));
        }
        let set = self.clone();
        #[cfg(feature = "profiling")]
        let set = {
            let mut set = set;
            set.query_profile = None;
            set
        };
        count_and_sample_single_l_reservoir(rng, set)
    }

    /// Sample up to `requested` entities uniformly from this set. If the set
    /// has fewer than `requested` entities, the entire set is returned.
    #[must_use]
    pub fn sample_entities<R>(&self, rng: &mut R, requested: usize) -> Vec<EntityId<E>>
    where
        R: Rng,
    {
        #[cfg(feature = "profiling")]
        let _query_profile_scope = self
            .query_profile
            .map(crate::profiling::QueryProfileHandle::scope);

        match self.try_len_impl() {
            Some(0) => {
                warn!("Requested a sample of entities from an empty population");
                vec![]
            }
            Some(_) => {
                let set = self.clone();
                #[cfg(feature = "profiling")]
                let set = {
                    let mut set = set;
                    set.query_profile = None;
                    set
                };
                sample_multiple_from_known_length(rng, set, requested)
            }
            None => {
                let set = self.clone();
                #[cfg(feature = "profiling")]
                let set = {
                    let mut set = set;
                    set.query_profile = None;
                    set
                };
                sample_multiple_l_reservoir(rng, set, requested)
            }
        }
    }

    /// Returns `Some(length)` only when the set length is trivially known.
    ///
    /// This is true only for direct `SourceSet` leaves except `PropertySet`.
    /// Composite expressions return `None`.
    #[must_use]
    pub fn try_len(&self) -> Option<usize> {
        #[cfg(feature = "profiling")]
        let _query_profile_scope = self
            .query_profile
            .map(crate::profiling::QueryProfileHandle::scope);
        self.try_len_impl()
    }

    fn try_len_impl(&self) -> Option<usize> {
        match &self.inner {
            EntitySetInner::Source(source) => source.try_len(),
            _ => None,
        }
    }

    fn try_nth(&self, idx: usize) -> Option<EntityId<E>> {
        match &self.inner {
            EntitySetInner::Source(source) => source.try_nth(idx),
            _ => None,
        }
    }

    fn as_range(&self) -> Option<Range<usize>> {
        match &self.inner {
            EntitySetInner::Source(SourceSet::PopulationRange(range)) => Some(range.clone()),
            _ => None,
        }
    }

    fn is_empty(&self) -> bool {
        match &self.inner {
            EntitySetInner::Source(SourceSet::PopulationRange(range)) => range.is_empty(),
            _ => false,
        }
    }

    fn sort_key(&self) -> (usize, u8) {
        match &self.inner {
            EntitySetInner::Source(source) => source.sort_key(),
            EntitySetInner::Union(left, right) => {
                // Union upper bound is additive; cost hint tracks the cheaper side.
                let (left_upper, left_hint) = left.sort_key();
                let (right_upper, right_hint) = right.sort_key();
                (
                    left_upper.saturating_add(right_upper),
                    left_hint.min(right_hint),
                )
            }
            EntitySetInner::Intersection(sets) => {
                let mut upper = usize::MAX;
                let mut hint = 0u8;
                for set in sets {
                    let (set_upper, set_hint) = set.sort_key();
                    upper = upper.min(set_upper);
                    hint = hint.saturating_add(set_hint);
                }
                if upper == usize::MAX {
                    upper = 0;
                }
                (upper, hint)
            }
            EntitySetInner::Difference(left, right) => {
                let (left_upper, left_hint) = left.sort_key();
                let (_, right_hint) = right.sort_key();
                (left_upper, left_hint.saturating_add(right_hint))
            }
        }
    }

    /// Structural equality check: same tree shape with same sources at leaves.
    fn structurally_eq(&self, other: &Self) -> bool {
        match (&self.inner, &other.inner) {
            (EntitySetInner::Source(a), EntitySetInner::Source(b)) => a == b,
            (EntitySetInner::Union(a1, a2), EntitySetInner::Union(b1, b2))
            | (EntitySetInner::Difference(a1, a2), EntitySetInner::Difference(b1, b2)) => {
                a1.structurally_eq(b1) && a2.structurally_eq(b2)
            }
            (EntitySetInner::Intersection(a_sets), EntitySetInner::Intersection(b_sets)) => {
                a_sets.len() == b_sets.len()
                    && a_sets
                        .iter()
                        .zip(b_sets.iter())
                        .all(|(a_set, b_set)| a_set.structurally_eq(b_set))
            }
            _ => false,
        }
    }
}

impl<'a, E: Entity> IntoIterator for EntitySet<'a, E> {
    type Item = EntityId<E>;
    type IntoIter = EntitySetIterator<'a, E>;

    fn into_iter(self) -> Self::IntoIter {
        EntitySetIterator::new(self)
    }
}

#[cfg(test)]
mod tests {
    use rand::rngs::StdRng;
    use rand::SeedableRng;

    use super::*;
    use crate::entity::ContextEntitiesExt;
    use crate::hashing::IndexSet;
    use crate::{
        define_derived_property, define_entity, define_multi_property, define_property, with,
        Context,
    };

    define_entity!(Person);
    define_property!(struct Age(u8), Person);
    define_property!(struct County(u32), Person, default_const = County(0));
    define_derived_property!(struct Senior(bool), Person, [Age], |age| Senior(age.0 >= 65));
    define_multi_property!(Person, (Age, County));
    define_multi_property!(Person, (County, Age));

    fn finite_set(ids: &[usize]) -> IndexSet<EntityId<Person>> {
        ids.iter()
            .copied()
            .map(EntityId::<Person>::new)
            .collect::<IndexSet<_>>()
    }

    fn as_entity_set(set: &IndexSet<EntityId<Person>>) -> EntitySet<Person> {
        EntitySet::from_source(SourceSet::IndexSet(set))
    }

    #[test]
    fn from_source_empty_is_empty() {
        let es = EntitySet::<Person>::empty();
        assert_eq!(es.sort_key().0, 0);
        for value in 0..10 {
            assert!(!es.contains(EntityId::<Person>::new(value)));
        }
    }

    #[test]
    fn from_source_population_ranges() {
        let population = EntitySet::from_source(SourceSet::<Person>::population_range(0..3));
        assert!(population.contains(EntityId::<Person>::new(0)));
        assert!(population.contains(EntityId::<Person>::new(2)));
        assert!(!population.contains(EntityId::<Person>::new(3)));
        assert_eq!(population.sort_key().0, 3);

        let singleton = EntitySet::from_source(SourceSet::<Person>::singleton(EntityId::new(5)));
        assert!(singleton.contains(EntityId::<Person>::new(5)));
        assert!(!singleton.contains(EntityId::<Person>::new(4)));
        assert_eq!(singleton.sort_key().0, 1);

        let range = EntitySet::from_source(SourceSet::<Person>::population_range(2..5));
        assert!(range.contains(EntityId::<Person>::new(2)));
        assert!(range.contains(EntityId::<Person>::new(4)));
        assert!(!range.contains(EntityId::<Person>::new(1)));
        assert!(!range.contains(EntityId::<Person>::new(5)));
        assert_eq!(range.try_len(), Some(3));
    }

    #[test]
    fn union_basic_behavior_without_legacy_reductions() {
        let a = finite_set(&[1, 2, 3]);
        let e = EntitySet::<Person>::empty();
        let u = EntitySet::from_source(SourceSet::<Person>::population_range(0..10));

        let a_union_empty = as_entity_set(&a).union(e);
        assert!(a_union_empty.contains(EntityId::<Person>::new(1)));
        assert!(!a_union_empty.contains(EntityId::<Person>::new(4)));

        let u_union_a = u.union(as_entity_set(&a));
        assert!(u_union_a.contains(EntityId::<Person>::new(0)));
        assert!(u_union_a.contains(EntityId::<Person>::new(9)));
        assert!(!u_union_a.contains(EntityId::<Person>::new(10)));
    }

    #[test]
    fn union_range_optimizations() {
        let adjacent = EntitySet::from_source(SourceSet::<Person>::population_range(0..3)).union(
            EntitySet::from_source(SourceSet::<Person>::population_range(3..6)),
        );
        assert!(matches!(
            adjacent,
            EntitySet { inner: EntitySetInner::Source(SourceSet::PopulationRange(ref range)), .. } if range == &(0..6)
        ));

        let overlapping = EntitySet::from_source(SourceSet::<Person>::population_range(2..6))
            .union(EntitySet::from_source(
                SourceSet::<Person>::population_range(4..8),
            ));
        assert!(matches!(
            overlapping,
            EntitySet { inner: EntitySetInner::Source(SourceSet::PopulationRange(ref range)), .. } if range == &(2..8)
        ));

        let disjoint = EntitySet::from_source(SourceSet::<Person>::singleton(EntityId::new(1)))
            .union(EntitySet::from_source(SourceSet::<Person>::singleton(
                EntityId::new(4),
            )));
        assert!(matches!(
            disjoint,
            EntitySet {
                inner: EntitySetInner::Union(_, _),
                ..
            }
        ));
    }

    #[test]
    fn intersection_range_optimizations() {
        let overlap = EntitySet::from_source(SourceSet::<Person>::population_range(2..6))
            .intersection(EntitySet::from_source(
                SourceSet::<Person>::population_range(4..8),
            ));
        assert!(matches!(
            overlap,
            EntitySet { inner: EntitySetInner::Source(SourceSet::PopulationRange(ref range)), .. } if range == &(4..6)
        ));

        let empty = EntitySet::from_source(SourceSet::<Person>::population_range(1..3))
            .intersection(EntitySet::from_source(
                SourceSet::<Person>::population_range(5..7),
            ));
        assert!(matches!(
            empty,
            EntitySet { inner: EntitySetInner::Source(SourceSet::PopulationRange(ref range)), .. } if range == &(0..0)
        ));

        let indexed_ids = finite_set(&[1, 2, 3]);
        let mixed = EntitySet::from_source(SourceSet::<Person>::singleton(EntityId::new(2)))
            .intersection(as_entity_set(&indexed_ids));
        assert!(mixed.contains(EntityId::<Person>::new(2)));
        assert!(!mixed.contains(EntityId::<Person>::new(1)));
    }

    #[test]
    fn difference_range_optimizations() {
        let unchanged = EntitySet::from_source(SourceSet::<Person>::population_range(2..6))
            .difference(EntitySet::from_source(
                SourceSet::<Person>::population_range(8..10),
            ));
        assert!(matches!(
            unchanged,
            EntitySet { inner: EntitySetInner::Source(SourceSet::PopulationRange(ref range)), .. } if range == &(2..6)
        ));

        let empty = EntitySet::from_source(SourceSet::<Person>::population_range(2..6)).difference(
            EntitySet::from_source(SourceSet::<Person>::population_range(1..7)),
        );
        assert!(matches!(
            empty,
            EntitySet { inner: EntitySetInner::Source(SourceSet::PopulationRange(ref range)), .. } if range == &(0..0)
        ));

        let trim_left = EntitySet::from_source(SourceSet::<Person>::population_range(2..6))
            .difference(EntitySet::from_source(
                SourceSet::<Person>::population_range(1..4),
            ));
        assert!(matches!(
            trim_left,
            EntitySet { inner: EntitySetInner::Source(SourceSet::PopulationRange(ref range)), .. } if range == &(4..6)
        ));

        let trim_right = EntitySet::from_source(SourceSet::<Person>::population_range(2..6))
            .difference(EntitySet::from_source(
                SourceSet::<Person>::population_range(4..8),
            ));
        assert!(matches!(
            trim_right,
            EntitySet { inner: EntitySetInner::Source(SourceSet::PopulationRange(ref range)), .. } if range == &(2..4)
        ));

        let split = EntitySet::from_source(SourceSet::<Person>::population_range(2..8)).difference(
            EntitySet::from_source(SourceSet::<Person>::population_range(4..6)),
        );
        assert!(matches!(
            split,
            EntitySet {
                inner: EntitySetInner::Difference(_, _),
                ..
            }
        ));
        assert!(split.contains(EntityId::<Person>::new(2)));
        assert!(split.contains(EntityId::<Person>::new(7)));
        assert!(!split.contains(EntityId::<Person>::new(4)));
        assert!(!split.contains(EntityId::<Person>::new(5)));
    }

    #[test]
    fn difference_is_not_commutative() {
        let a = finite_set(&[1, 2, 3]);
        let b = finite_set(&[2, 3, 4]);

        let d1 = as_entity_set(&a).difference(as_entity_set(&b));
        let c = finite_set(&[2, 3, 4]);
        let d = finite_set(&[1, 2, 3]);
        let d2 = as_entity_set(&c).difference(as_entity_set(&d));

        assert!(d1.contains(EntityId::<Person>::new(1)));
        assert!(!d1.contains(EntityId::<Person>::new(4)));
        assert!(d2.contains(EntityId::<Person>::new(4)));
        assert!(!d2.contains(EntityId::<Person>::new(1)));
    }

    #[test]
    fn sort_key_rules() {
        let a = finite_set(&[1, 2]);
        let b = finite_set(&[2, 3, 4]);

        let union = as_entity_set(&a).union(as_entity_set(&b));
        assert_eq!(union.sort_key(), (a.len() + b.len(), 3));

        let intersection = as_entity_set(&a).intersection(as_entity_set(&b));
        assert_eq!(intersection.sort_key(), (a.len().min(b.len()), 6));

        let difference = as_entity_set(&a).difference(as_entity_set(&b));
        assert_eq!(difference.sort_key(), (a.len(), 6));
    }

    #[test]
    fn compound_expressions_membership() {
        let a = finite_set(&[1, 2, 3, 4]);
        let b = finite_set(&[3, 4, 5]);
        let c = finite_set(&[10, 20]);
        let d = finite_set(&[20]);

        let union_of_intersections = as_entity_set(&a)
            .intersection(as_entity_set(&b))
            .union(as_entity_set(&c).intersection(as_entity_set(&d)));
        assert!(union_of_intersections.contains(EntityId::<Person>::new(3)));
        assert!(union_of_intersections.contains(EntityId::<Person>::new(4)));
        assert!(union_of_intersections.contains(EntityId::<Person>::new(20)));
        assert!(!union_of_intersections.contains(EntityId::<Person>::new(5)));

        let a2 = finite_set(&[1, 2, 3]);
        let b2 = finite_set(&[3, 4, 5]);
        let a3 = finite_set(&[1, 2, 3]);
        let law = as_entity_set(&a3).intersection(as_entity_set(&a2).union(as_entity_set(&b2)));
        assert!(law.contains(EntityId::<Person>::new(1)));
        assert!(law.contains(EntityId::<Person>::new(2)));
        assert!(law.contains(EntityId::<Person>::new(3)));
        assert!(!law.contains(EntityId::<Person>::new(4)));
    }

    #[test]
    fn clone_preserves_composite_expression_behavior() {
        let a = finite_set(&[1, 2, 3, 4]);
        let b = finite_set(&[3, 4, 5]);
        let c = finite_set(&[2]);

        let set = as_entity_set(&a)
            .difference(as_entity_set(&c))
            .union(as_entity_set(&b));
        let cloned = set.clone();

        for value in 0..7 {
            let entity_id = EntityId::<Person>::new(value);
            assert_eq!(set.contains(entity_id), cloned.contains(entity_id));
        }

        assert_eq!(
            set.into_iter().collect::<Vec<_>>(),
            cloned.into_iter().collect::<Vec<_>>()
        );
    }

    #[test]
    fn population_zero_is_empty() {
        let es = EntitySet::from_source(SourceSet::<Person>::empty());
        assert_eq!(es.sort_key().0, 0);
        assert!(!es.contains(EntityId::<Person>::new(0)));
    }

    #[test]
    fn try_len_known_only_for_non_property_sources() {
        let empty = EntitySet::<Person>::from_source(SourceSet::empty());
        assert_eq!(empty.try_len(), Some(0));

        let singleton = EntitySet::<Person>::from_source(SourceSet::singleton(EntityId::new(42)));
        assert_eq!(singleton.try_len(), Some(1));

        let population = EntitySet::<Person>::from_source(SourceSet::population_range(0..5));
        assert_eq!(population.try_len(), Some(5));

        let range = EntitySet::<Person>::from_source(SourceSet::population_range(4..9));
        assert_eq!(range.try_len(), Some(5));

        let index_data = [EntityId::new(1), EntityId::new(2), EntityId::new(3)]
            .into_iter()
            .collect::<IndexSet<_>>();
        let indexed = EntitySet::<Person>::from_source(SourceSet::IndexSet(&index_data));
        assert_eq!(indexed.try_len(), Some(3));

        let mut context = Context::new();
        context.add_entity(with!(Person, Age(10))).unwrap();
        let property_source = SourceSet::<Person>::new(Age(10), &context).unwrap();
        assert!(matches!(property_source, SourceSet::PropertySet(_)));
        let property_set = EntitySet::<Person>::from_source(property_source);
        assert_eq!(property_set.try_len(), None);

        let composed = EntitySet::<Person>::from_source(SourceSet::population_range(0..3))
            .difference(EntitySet::from_source(SourceSet::singleton(EntityId::new(
                1,
            ))));
        assert_eq!(composed.try_len(), None);
    }

    #[test]
    fn range_leaf_works_inside_composite_expressions() {
        let indexed_ids = finite_set(&[1, 3, 5, 8]);
        let indexed = as_entity_set(&indexed_ids);
        let range = EntitySet::from_source(SourceSet::<Person>::population_range(2..8));

        let intersection = range.intersection(indexed);
        assert!(!intersection.contains(EntityId::new(1)));
        assert!(intersection.contains(EntityId::new(3)));
        assert!(intersection.contains(EntityId::new(5)));
        assert!(!intersection.contains(EntityId::new(8)));
    }

    #[test]
    fn clone_preserves_unindexed_concrete_property_query_results() {
        let mut context = Context::new();
        let p1 = context.add_entity(with!(Person, Age(10))).unwrap();
        let p2 = context.add_entity(with!(Person, Age(10))).unwrap();
        let _p3 = context.add_entity(with!(Person, Age(11))).unwrap();

        let set = context.query::<Person, _>(with!(Person, Age(10)));
        assert_eq!(set.try_len(), None);
        let cloned = set.clone();

        let mut iter = set.into_iter();
        assert_eq!(iter.next(), Some(p1));
        assert_eq!(iter.collect::<Vec<_>>(), vec![p2]);

        assert!(cloned.contains(p1));
        assert!(cloned.contains(p2));
        assert_eq!(cloned.into_iter().collect::<Vec<_>>(), vec![p1, p2]);
    }

    #[test]
    fn clone_preserves_unindexed_derived_property_query_results() {
        let mut context = Context::new();
        let _p1 = context.add_entity(with!(Person, Age(64))).unwrap();
        let p2 = context.add_entity(with!(Person, Age(65))).unwrap();
        let p3 = context.add_entity(with!(Person, Age(90))).unwrap();

        let set = context.query::<Person, _>(with!(Person, Senior(true)));
        assert_eq!(set.try_len(), None);
        let cloned = set.clone();

        assert!(set.contains(p2));
        assert!(set.contains(p3));
        assert_eq!(set.into_iter().collect::<Vec<_>>(), vec![p2, p3]);
        assert_eq!(cloned.into_iter().collect::<Vec<_>>(), vec![p2, p3]);
    }

    #[test]
    fn union_of_same_unindexed_property_query_deduplicates_to_one_source() {
        let mut context = Context::new();
        let p1 = context.add_entity(with!(Person, Age(10))).unwrap();
        let p2 = context.add_entity(with!(Person, Age(10))).unwrap();
        let _p3 = context.add_entity(with!(Person, Age(11))).unwrap();

        let query = context.query::<Person, _>(with!(Person, Age(10)));
        let union = query.clone().union(query);

        assert!(matches!(
            union,
            EntitySet {
                inner: EntitySetInner::Source(SourceSet::PropertySet(_)),
                ..
            }
        ));
        assert_eq!(union.into_iter().collect::<Vec<_>>(), vec![p1, p2]);
    }

    #[test]
    fn union_of_equivalent_unindexed_multi_property_queries_deduplicates_to_one_source() {
        let mut context = Context::new();
        let matching = context
            .add_entity(with!(Person, Age(28), County(7)))
            .unwrap();
        let _wrong_county = context
            .add_entity(with!(Person, Age(28), County(8)))
            .unwrap();
        let _wrong_age = context
            .add_entity(with!(Person, Age(29), County(7)))
            .unwrap();

        let age_county = context.query::<Person, _>(with!(Person, (Age(28), County(7))));
        let county_age = context.query::<Person, _>(with!(Person, (County(7), Age(28))));
        let union = age_county.union(county_age);

        assert!(matches!(
            union,
            EntitySet {
                inner: EntitySetInner::Source(SourceSet::PropertySet(_)),
                ..
            }
        ));
        assert_eq!(union.into_iter().collect::<Vec<_>>(), vec![matching]);
    }

    #[test]
    fn sample_entity_excluding_skips_excluded() {
        let set = EntitySet::from_source(SourceSet::<Person>::PopulationRange(0..5));
        let excluded = EntityId::<Person>::new(2);
        let mut rng = StdRng::seed_from_u64(42);
        for _ in 0..200 {
            let sampled = set.sample_entity_excluding(&mut rng, excluded).unwrap();
            assert_ne!(sampled, excluded);
            assert!(sampled.0 < 5);
        }
    }

    #[test]
    fn sample_entity_excluding_returns_none_when_only_excluded_present() {
        let only = EntityId::<Person>::new(7);
        let single = finite_set(&[7]);
        let mut rng = StdRng::seed_from_u64(42);
        assert_eq!(
            as_entity_set(&single).sample_entity_excluding(&mut rng, only),
            None
        );
    }

    #[test]
    fn sample_entity_excluding_returns_none_on_empty() {
        let mut rng = StdRng::seed_from_u64(42);
        assert_eq!(
            EntitySet::<Person>::empty()
                .sample_entity_excluding(&mut rng, EntityId::<Person>::new(0)),
            None
        );
    }

    #[test]
    fn sample_entity_excluding_excluded_not_in_set_uses_first_pick() {
        // When `excluded` is outside the set, every sample is the first
        // uniform pick — exercises the no-resample path.
        let set = EntitySet::from_source(SourceSet::<Person>::PopulationRange(0..10));
        let mut rng = StdRng::seed_from_u64(42);
        for _ in 0..200 {
            let sampled = set
                .sample_entity_excluding(&mut rng, EntityId::<Person>::new(999))
                .unwrap();
            assert!(sampled.0 < 10);
        }
    }

    #[test]
    fn sample_entity_excluding_uniform_over_known_length() {
        // Chi-square test on PopulationRange (known-length, fast path).
        let excluded = EntityId::<Person>::new(7);
        let set = EntitySet::from_source(SourceSet::<Person>::PopulationRange(0..20));
        let num_runs = 50_000;
        let mut counts = [0usize; 20];
        let mut rng = StdRng::seed_from_u64(42);
        for _ in 0..num_runs {
            let id = set.sample_entity_excluding(&mut rng, excluded).unwrap();
            counts[id.0] += 1;
        }
        assert_eq!(counts[excluded.0], 0);

        let expected = num_runs as f64 / 19.0;
        let chi_square: f64 = counts
            .iter()
            .enumerate()
            .filter(|(i, _)| *i != excluded.0)
            .map(|(_, &obs)| {
                let diff = obs as f64 - expected;
                diff * diff / expected
            })
            .sum();
        // df = 18, χ²_{0.999} ≈ 42.31
        assert!(chi_square < 42.31, "χ² = {chi_square}");
    }
}
