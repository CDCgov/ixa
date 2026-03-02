//! A lazy, composable set type built from set-algebraic expressions.
//!
//! An [`EntitySet`] represents a set of [`EntityId`] values as a tree of union,
//! intersection, and difference operations over leaf [`SourceSet`] nodes. The tree
//! is constructed eagerly but evaluated lazily: membership tests ([`contains`]) walk
//! the tree on demand, and iteration is deferred to [`EntitySetIterator`].
//!
//! Construction methods apply algebraic simplifications (e.g. `A ∪ ∅ = A`,
//! `A ∩ A = A`) and reorder operands by estimated size to improve short-circuit
//! performance.
//!
//! [`contains`]: EntitySet::contains

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
        EntitySet(EntitySetInner::Source(SourceSet::Empty))
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
        // Identity: A ∪ ∅ = A, ∅ ∪ B = B
        if self.is_empty() {
            return other;
        }
        if other.is_empty() {
            return self;
        }
        // Idempotence: A ∪ A = A  (same structure over same sources)
        if self.structurally_eq(&other) {
            return self;
        }
        // Universal absorption: U ∪ A = U, A ∪ U = U
        if self.is_universal() {
            return self;
        }
        if other.is_universal() {
            return other;
        }
        // Singleton absorption: {e} ∪ A = A if e ∈ A
        if let Some(e) = self.as_singleton() {
            if other.contains(e) {
                return other;
            }
        }
        if let Some(e) = other.as_singleton() {
            if self.contains(e) {
                return self;
            }
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
        // Annihilator: A ∩ ∅ = ∅
        if self.is_empty() || other.is_empty() {
            return Self::empty();
        }
        // Idempotence: A ∩ A = A
        if self.structurally_eq(&other) {
            return self;
        }
        // Identity: U ∩ A = A
        if self.is_universal() {
            return other;
        }
        if other.is_universal() {
            return self;
        }
        // Singleton restriction:
        // {e} ∩ A = {e} if e ∈ A, otherwise ∅
        if let Some(e) = self.as_singleton() {
            return if other.contains(e) {
                self
            } else {
                Self::empty()
            };
        }
        if let Some(e) = other.as_singleton() {
            return if self.contains(e) {
                other
            } else {
                Self::empty()
            };
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
        // Identity: A \ ∅ = A
        if other.is_empty() {
            return self;
        }
        // Annihilator: ∅ \ B = ∅
        if self.is_empty() {
            return Self::empty();
        }
        // Self-subtraction: A \ A = ∅
        if self.structurally_eq(&other) {
            return Self::empty();
        }
        // Universal subtraction: A \ U = ∅
        if other.is_universal() {
            return Self::empty();
        }
        // Singleton restriction:
        // {e} \ A = {e} if e ∉ A, otherwise ∅
        if let Some(e) = self.as_singleton() {
            return if other.contains(e) {
                Self::empty()
            } else {
                self
            };
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

    /// Returns `true` if this set is the abstract empty set (`∅`).
    ///
    /// A return value of `false` does not guarantee the set is non-empty.
    /// For example, it may be an intersection of disjoint sets.
    fn is_empty(&self) -> bool {
        matches!(self, EntitySet(EntitySetInner::Source(SourceSet::Empty)))
    }
    /// Returns `true` if this set represents the entire entity population.
    fn is_universal(&self) -> bool {
        matches!(
            self,
            EntitySet(EntitySetInner::Source(SourceSet::Population(_)))
        )
    }
    /// Returns the contained entity id if this set is a singleton leaf.
    fn as_singleton(&self) -> Option<EntityId<E>> {
        match self {
            EntitySet(EntitySetInner::Source(SourceSet::Entity(e))) => Some(*e),
            _ => None,
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
    use std::cell::RefCell;

    use super::*;
    use crate::entity::ContextEntitiesExt;
    use crate::hashing::IndexSet;
    use crate::{define_entity, define_property, Context};

    define_entity!(Person);
    define_property!(struct Age(u8), Person);

    fn finite_set(ids: &[usize]) -> RefCell<IndexSet<EntityId<Person>>> {
        RefCell::new(
            ids.iter()
                .copied()
                .map(EntityId::<Person>::new)
                .collect::<IndexSet<_>>(),
        )
    }

    fn as_entity_set(set: &RefCell<IndexSet<EntityId<Person>>>) -> EntitySet<Person> {
        EntitySet::from_source(SourceSet::IndexSet(set.borrow()))
    }

    #[test]
    fn from_source_empty_is_empty() {
        let es = EntitySet::<Person>::from_source(SourceSet::Empty);
        assert_eq!(es.sort_key().0, 0);
        for value in 0..10 {
            assert!(!es.contains(EntityId::<Person>::new(value)));
        }
    }

    #[test]
    fn from_source_entity_and_population() {
        let entity =
            EntitySet::from_source(SourceSet::<Person>::Entity(EntityId::<Person>::new(5)));
        assert!(entity.contains(EntityId::<Person>::new(5)));
        assert!(!entity.contains(EntityId::<Person>::new(4)));
        assert_eq!(entity.sort_key().0, 1);

        let population = EntitySet::from_source(SourceSet::<Person>::Population(3));
        assert!(population.contains(EntityId::<Person>::new(0)));
        assert!(population.contains(EntityId::<Person>::new(2)));
        assert!(!population.contains(EntityId::<Person>::new(3)));
        assert_eq!(population.sort_key().0, 3);
    }

    #[test]
    fn union_algebraic_reductions() {
        let a = finite_set(&[1, 2, 3]);
        let e = EntitySet::<Person>::empty();
        let u = EntitySet::from_source(SourceSet::<Person>::Population(10));

        let a_union_empty = as_entity_set(&a).union(e);
        assert!(a_union_empty.contains(EntityId::<Person>::new(1)));
        assert!(!a_union_empty.contains(EntityId::<Person>::new(4)));

        let u_union_a = u.union(as_entity_set(&a));
        assert!(matches!(
            u_union_a,
            EntitySet(EntitySetInner::Source(SourceSet::Population(10)))
        ));
    }

    #[test]
    fn union_entity_absorption() {
        let a = finite_set(&[1, 2, 3]);
        let absorbed =
            EntitySet::from_source(SourceSet::<Person>::Entity(EntityId::<Person>::new(2)))
                .union(as_entity_set(&a));
        assert!(absorbed.contains(EntityId::<Person>::new(1)));
        assert!(absorbed.contains(EntityId::<Person>::new(2)));
        assert!(absorbed.contains(EntityId::<Person>::new(3)));

        let b = finite_set(&[1, 2, 3]);
        let not_absorbed =
            EntitySet::from_source(SourceSet::<Person>::Entity(EntityId::<Person>::new(8)))
                .union(as_entity_set(&b));
        assert!(not_absorbed.contains(EntityId::<Person>::new(8)));
        assert!(not_absorbed.contains(EntityId::<Person>::new(1)));
    }

    #[test]
    fn intersection_algebraic_reductions() {
        let a = finite_set(&[1, 2, 3]);
        let u = EntitySet::from_source(SourceSet::<Person>::Population(10));

        let a_inter_u = as_entity_set(&a).intersection(u);
        assert!(a_inter_u.contains(EntityId::<Person>::new(1)));
        assert!(a_inter_u.contains(EntityId::<Person>::new(2)));
        assert!(!a_inter_u.contains(EntityId::<Person>::new(9)));

        let b = finite_set(&[1, 2, 3]);
        let present =
            EntitySet::from_source(SourceSet::<Person>::Entity(EntityId::<Person>::new(2)))
                .intersection(as_entity_set(&b));
        assert!(matches!(
            present,
            EntitySet(EntitySetInner::Source(SourceSet::Entity(_)))
        ));

        let c = finite_set(&[1, 2, 3]);
        let absent =
            EntitySet::from_source(SourceSet::<Person>::Entity(EntityId::<Person>::new(7)))
                .intersection(as_entity_set(&c));
        assert!(!absent.contains(EntityId::<Person>::new(7)));
    }

    #[test]
    fn difference_algebraic_reductions() {
        let a = finite_set(&[1, 2, 3]);

        let minus_empty = as_entity_set(&a).difference(EntitySet::empty());
        assert!(minus_empty.contains(EntityId::<Person>::new(1)));
        assert!(!minus_empty.contains(EntityId::<Person>::new(9)));

        let minus_universe =
            as_entity_set(&a)
                .difference(EntitySet::from_source(SourceSet::<Person>::Population(10)));
        for value in 0..10 {
            assert!(!minus_universe.contains(EntityId::<Person>::new(value)));
        }

        let b = finite_set(&[1, 2, 3]);
        let singleton_absent =
            EntitySet::from_source(SourceSet::<Person>::Entity(EntityId::<Person>::new(8)))
                .difference(as_entity_set(&b));
        assert!(singleton_absent.contains(EntityId::<Person>::new(8)));

        let c = finite_set(&[1, 2, 3]);
        let singleton_present =
            EntitySet::from_source(SourceSet::<Person>::Entity(EntityId::<Person>::new(2)))
                .difference(as_entity_set(&c));
        assert!(!singleton_present.contains(EntityId::<Person>::new(2)));
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
        assert_eq!(union.sort_key(), (a.borrow().len() + b.borrow().len(), 3));

        let intersection = as_entity_set(&a).intersection(as_entity_set(&b));
        assert_eq!(
            intersection.sort_key(),
            (a.borrow().len().min(b.borrow().len()), 6)
        );

        let difference = as_entity_set(&a).difference(as_entity_set(&b));
        assert_eq!(difference.sort_key(), (a.borrow().len(), 6));
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
    fn population_zero_is_empty() {
        let es = EntitySet::from_source(SourceSet::<Person>::Population(0));
        assert_eq!(es.sort_key().0, 0);
        assert!(!es.contains(EntityId::<Person>::new(0)));
    }

    #[test]
    fn try_len_known_only_for_non_property_sources() {
        let empty = EntitySet::<Person>::from_source(SourceSet::Empty);
        assert_eq!(empty.try_len(), Some(0));

        let singleton = EntitySet::<Person>::from_source(SourceSet::Entity(EntityId::new(42)));
        assert_eq!(singleton.try_len(), Some(1));

        let population = EntitySet::<Person>::from_source(SourceSet::Population(5));
        assert_eq!(population.try_len(), Some(5));

        let index_data = RefCell::new(
            [EntityId::new(1), EntityId::new(2), EntityId::new(3)]
                .into_iter()
                .collect::<IndexSet<_>>(),
        );
        let indexed = EntitySet::<Person>::from_source(SourceSet::IndexSet(index_data.borrow()));
        assert_eq!(indexed.try_len(), Some(3));

        let mut context = Context::new();
        context.add_entity((Age(10),)).unwrap();
        let property_source = SourceSet::<Person>::new(Age(10), &context).unwrap();
        assert!(matches!(property_source, SourceSet::PropertySet(_)));
        let property_set = EntitySet::<Person>::from_source(property_source);
        assert_eq!(property_set.try_len(), None);

        let composed = EntitySet::<Person>::from_source(SourceSet::Population(3))
            .difference(EntitySet::from_source(SourceSet::Entity(EntityId::new(1))));
        assert_eq!(composed.try_len(), None);
    }
}
