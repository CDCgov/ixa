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

use std::ops::Range;

use super::{EntitySetIterator, SourceSet};
use crate::entity::{Entity, EntityId};

/// Opaque public wrapper around the internal set-expression tree.
pub struct EntitySet<'a, E: Entity>(EntitySetInner<'a, E>);

/// Internal set-expression tree used to represent composed query sources.
pub(super) enum EntitySetInner<'a, E: Entity> {
    Source(SourceSet<'a, E>),
    Union(Box<EntitySet<'a, E>>, Box<EntitySet<'a, E>>),
    Intersection(Vec<EntitySet<'a, E>>),
    Difference(Box<EntitySet<'a, E>>, Box<EntitySet<'a, E>>),
}

impl<'a, E: Entity> Clone for EntitySet<'a, E> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
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
    pub(super) fn into_inner(self) -> EntitySetInner<'a, E> {
        self.0
    }

    pub(super) fn is_source_leaf(&self) -> bool {
        matches!(self.0, EntitySetInner::Source(_))
    }

    pub(super) fn into_source_leaf(self) -> Option<SourceSet<'a, E>> {
        match self.0 {
            EntitySetInner::Source(source) => Some(source),
            _ => None,
        }
    }

    /// Create an empty entity set.
    pub fn empty() -> Self {
        EntitySet(EntitySetInner::Source(SourceSet::empty()))
    }

    /// Create an entity set from a single source set.
    pub(crate) fn from_source(source: SourceSet<'a, E>) -> Self {
        EntitySet(EntitySetInner::Source(source))
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

        EntitySet(EntitySetInner::Intersection(sets))
    }

    pub fn union(self, other: Self) -> Self {
        // Idempotence: A ∪ A = A  (same structure over same sources)
        if self.structurally_eq(&other) {
            return self;
        }

        // Adjacent or overlapping intervals
        if let (Some(a), Some(b)) = (self.as_range(), other.as_range()) {
            if a.start <= b.end && b.start <= a.end {
                return Self::from_source(SourceSet::population_range(
                    a.start.min(b.start)..a.end.max(b.end),
                ));
            }
        }

        // Union with empty set is identity: A ∪ ∅ = ∅ ∪ A = A
        match (self.is_empty(), other.is_empty()) {
            (true, _) => return other,
            (_, true) => return self,
            _ => { /* pass */ }
        }

        // Larger set on LHS: more likely to short-circuit `||`.
        let (left, right) = if self.sort_key() >= other.sort_key() {
            (self, other)
        } else {
            (other, self)
        };
        EntitySet(EntitySetInner::Union(Box::new(left), Box::new(right)))
    }

    pub fn intersection(self, other: Self) -> Self {
        // Idempotence: A ∩ A = A
        if self.structurally_eq(&other) {
            return self;
        }

        // Intersection of overlapping intervals
        if let (Some(a), Some(b)) = (self.as_range(), other.as_range()) {
            let overlap = a.start.max(b.start)..a.end.min(b.end);
            return if overlap.is_empty() {
                Self::empty()
            } else {
                Self::from_source(SourceSet::population_range(overlap))
            };
        }

        // Intersection an empty set is empty: A ∩ ∅ = ∅ ∩ A = ∅
        match (self.is_empty(), other.is_empty()) {
            (true, _) => return self,
            (_, true) => return other,
            _ => { /* pass */ }
        }

        let mut sets = match self {
            EntitySet(EntitySetInner::Intersection(sets)) => sets,
            _ => vec![self],
        };

        sets.push(other);
        // Keep intersections sorted smallest-to-largest so iterators can take the
        // first source as the driver and membership checks short-circuit quickly.
        sets.sort_unstable_by_key(EntitySet::sort_key);
        EntitySet(EntitySetInner::Intersection(sets))
    }

    pub fn difference(self, other: Self) -> Self {
        // Self-subtraction: A \ A = ∅
        if self.structurally_eq(&other) {
            return Self::empty();
        }

        if let (Some(a), Some(b)) = (self.as_range(), other.as_range()) {
            let overlap = a.start.max(b.start)..a.end.min(b.end);
            // Disjoint ranges leave the left operand unchanged.
            if overlap.is_empty() {
                return Self::from_source(SourceSet::population_range(a));
            }
            // A covering subtraction removes the entire left range.
            if overlap.start == a.start && overlap.end == a.end {
                return Self::empty();
            }
            // Trimming the left edge still leaves one contiguous suffix.
            if overlap.start == a.start {
                return Self::from_source(SourceSet::population_range(overlap.end..a.end));
            }
            // Trimming the right edge still leaves one contiguous prefix.
            if overlap.end == a.end {
                return Self::from_source(SourceSet::population_range(a.start..overlap.start));
            }
            // An interior subtraction would split the range, so keep the generic difference node.
        }

        // Subtraction involving an empty set is identity: A \ ∅ = A, ∅ \ A = ∅
        if self.is_empty() || other.is_empty() {
            return self;
        }

        EntitySet(EntitySetInner::Difference(Box::new(self), Box::new(other)))
    }

    /// Test whether `entity_id` is a member of this set.
    pub fn contains(&self, entity_id: EntityId<E>) -> bool {
        match self {
            EntitySet(EntitySetInner::Source(source)) => source.contains(entity_id),
            EntitySet(EntitySetInner::Union(a, b)) => {
                a.contains(entity_id) || b.contains(entity_id)
            }
            EntitySet(EntitySetInner::Intersection(sets)) => {
                sets.iter().all(|set| set.contains(entity_id))
            }
            EntitySet(EntitySetInner::Difference(a, b)) => {
                a.contains(entity_id) && !b.contains(entity_id)
            }
        }
    }

    /// Collect this set's contents into an owned vector of `EntityId<E>`.
    pub fn to_owned_vec(self) -> Vec<EntityId<E>> {
        self.into_iter().collect()
    }

    /// Returns `Some(length)` only when the set length is trivially known.
    ///
    /// This is true only for direct `SourceSet` leaves except `PropertySet`.
    /// Composite expressions return `None`.
    pub fn try_len(&self) -> Option<usize> {
        match self {
            EntitySet(EntitySetInner::Source(source)) => source.try_len(),
            _ => None,
        }
    }

    fn as_range(&self) -> Option<Range<usize>> {
        match self {
            EntitySet(EntitySetInner::Source(SourceSet::PopulationRange(range))) => {
                Some(range.clone())
            }
            _ => None,
        }
    }

    fn is_empty(&self) -> bool {
        match self {
            EntitySet(EntitySetInner::Source(SourceSet::PopulationRange(range))) => {
                range.is_empty()
            }
            _ => false,
        }
    }

    fn sort_key(&self) -> (usize, u8) {
        match self {
            EntitySet(EntitySetInner::Source(source)) => source.sort_key(),
            EntitySet(EntitySetInner::Union(left, right)) => {
                // Union upper bound is additive; cost hint tracks the cheaper side.
                let (left_upper, left_hint) = left.sort_key();
                let (right_upper, right_hint) = right.sort_key();
                (
                    left_upper.saturating_add(right_upper),
                    left_hint.min(right_hint),
                )
            }
            EntitySet(EntitySetInner::Intersection(sets)) => {
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
            EntitySet(EntitySetInner::Difference(left, right)) => {
                let (left_upper, left_hint) = left.sort_key();
                let (_, right_hint) = right.sort_key();
                (left_upper, left_hint.saturating_add(right_hint))
            }
        }
    }

    /// Structural equality check: same tree shape with same sources at leaves.
    fn structurally_eq(&self, other: &Self) -> bool {
        match (self, other) {
            (EntitySet(EntitySetInner::Source(a)), EntitySet(EntitySetInner::Source(b))) => a == b,
            (
                EntitySet(EntitySetInner::Union(a1, a2)),
                EntitySet(EntitySetInner::Union(b1, b2)),
            )
            | (
                EntitySet(EntitySetInner::Difference(a1, a2)),
                EntitySet(EntitySetInner::Difference(b1, b2)),
            ) => a1.structurally_eq(b1) && a2.structurally_eq(b2),
            (
                EntitySet(EntitySetInner::Intersection(a_sets)),
                EntitySet(EntitySetInner::Intersection(b_sets)),
            ) => {
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
    use super::*;
    use crate::entity::ContextEntitiesExt;
    use crate::hashing::IndexSet;
    use crate::{define_derived_property, define_entity, define_property, Context};

    define_entity!(Person);
    define_property!(struct Age(u8), Person);
    define_derived_property!(struct Senior(bool), Person, [Age], |age| Senior(age.0 >= 65));

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
            EntitySet(EntitySetInner::Source(SourceSet::PopulationRange(ref range))) if range == &(0..6)
        ));

        let overlapping = EntitySet::from_source(SourceSet::<Person>::population_range(2..6))
            .union(EntitySet::from_source(
                SourceSet::<Person>::population_range(4..8),
            ));
        assert!(matches!(
            overlapping,
            EntitySet(EntitySetInner::Source(SourceSet::PopulationRange(ref range))) if range == &(2..8)
        ));

        let disjoint = EntitySet::from_source(SourceSet::<Person>::singleton(EntityId::new(1)))
            .union(EntitySet::from_source(SourceSet::<Person>::singleton(
                EntityId::new(4),
            )));
        assert!(matches!(disjoint, EntitySet(EntitySetInner::Union(_, _))));
    }

    #[test]
    fn intersection_range_optimizations() {
        let overlap = EntitySet::from_source(SourceSet::<Person>::population_range(2..6))
            .intersection(EntitySet::from_source(
                SourceSet::<Person>::population_range(4..8),
            ));
        assert!(matches!(
            overlap,
            EntitySet(EntitySetInner::Source(SourceSet::PopulationRange(ref range))) if range == &(4..6)
        ));

        let empty = EntitySet::from_source(SourceSet::<Person>::population_range(1..3))
            .intersection(EntitySet::from_source(
                SourceSet::<Person>::population_range(5..7),
            ));
        assert!(matches!(
            empty,
            EntitySet(EntitySetInner::Source(SourceSet::PopulationRange(ref range))) if range == &(0..0)
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
            EntitySet(EntitySetInner::Source(SourceSet::PopulationRange(ref range))) if range == &(2..6)
        ));

        let empty = EntitySet::from_source(SourceSet::<Person>::population_range(2..6)).difference(
            EntitySet::from_source(SourceSet::<Person>::population_range(1..7)),
        );
        assert!(matches!(
            empty,
            EntitySet(EntitySetInner::Source(SourceSet::PopulationRange(ref range))) if range == &(0..0)
        ));

        let trim_left = EntitySet::from_source(SourceSet::<Person>::population_range(2..6))
            .difference(EntitySet::from_source(
                SourceSet::<Person>::population_range(1..4),
            ));
        assert!(matches!(
            trim_left,
            EntitySet(EntitySetInner::Source(SourceSet::PopulationRange(ref range))) if range == &(4..6)
        ));

        let trim_right = EntitySet::from_source(SourceSet::<Person>::population_range(2..6))
            .difference(EntitySet::from_source(
                SourceSet::<Person>::population_range(4..8),
            ));
        assert!(matches!(
            trim_right,
            EntitySet(EntitySetInner::Source(SourceSet::PopulationRange(ref range))) if range == &(2..4)
        ));

        let split = EntitySet::from_source(SourceSet::<Person>::population_range(2..8)).difference(
            EntitySet::from_source(SourceSet::<Person>::population_range(4..6)),
        );
        assert!(matches!(split, EntitySet(EntitySetInner::Difference(_, _))));
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
        context.add_entity((Age(10),)).unwrap();
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
        let p1 = context.add_entity((Age(10),)).unwrap();
        let p2 = context.add_entity((Age(10),)).unwrap();
        let _p3 = context.add_entity((Age(11),)).unwrap();

        let set = context.query::<Person, _>((Age(10),));
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
        let _p1 = context.add_entity((Age(64),)).unwrap();
        let p2 = context.add_entity((Age(65),)).unwrap();
        let p3 = context.add_entity((Age(90),)).unwrap();

        let set = context.query::<Person, _>((Senior(true),));
        assert_eq!(set.try_len(), None);
        let cloned = set.clone();

        assert!(set.contains(p2));
        assert!(set.contains(p3));
        assert_eq!(set.into_iter().collect::<Vec<_>>(), vec![p2, p3]);
        assert_eq!(cloned.into_iter().collect::<Vec<_>>(), vec![p2, p3]);
    }
}
