use crate::people::PersonId;
use fxhash::FxBuildHasher;
use indexmap::set::IndexSet;
use rand::Rng;

#[derive(Clone)]
pub struct IndexSetPersonContainer {
    people: IndexSet<PersonId, FxBuildHasher>,
}

impl Default for IndexSetPersonContainer {
    fn default() -> Self {
        Self::new()
    }
}

impl IndexSetPersonContainer {
    #[must_use]
    pub fn new() -> IndexSetPersonContainer {
        IndexSetPersonContainer {
            people: IndexSet::with_hasher(FxBuildHasher::default()),
        }
    }

    #[must_use]
    pub fn with_capacity(n: usize) -> IndexSetPersonContainer {
        IndexSetPersonContainer {
            people: IndexSet::with_capacity_and_hasher(n, FxBuildHasher::default()),
        }
    }

    pub fn insert(&mut self, person_id: PersonId) {
        self.people.insert(person_id);
    }

    pub fn remove(&mut self, person_id: &PersonId) {
        self.people.swap_remove(person_id);
    }

    #[must_use]
    pub fn contains(&self, person_id: &PersonId) -> bool {
        self.people.contains(person_id)
    }

    #[allow(clippy::missing_panics_doc)]
    pub fn get_random(&self, rng: &mut impl Rng) -> Option<PersonId> {
        if self.people.is_empty() {
            return None;
        }
        Some(
            *self
                .people
                .get_index(rng.gen_range(0..self.people.len()))
                .unwrap(),
        )
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.people.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.people.is_empty()
    }
}

#[cfg(test)]
mod test {
    use rand::{rngs::StdRng, SeedableRng};

    use crate::people::PersonId;

    use super::IndexSetPersonContainer;

    #[test]
    fn new_remove_sample() {
        let mut container = IndexSetPersonContainer::new();
        let mut rng = StdRng::seed_from_u64(8_675_309);

        for i in 0..4 {
            container.insert(PersonId { id: i });
        }

        assert_eq!(container.len(), 4);
        let sample = container.get_random(&mut rng).unwrap().id;
        assert!(sample <= 3);

        container.remove(&PersonId { id: 0 });
        assert_eq!(container.len(), 3);
        let sample = container.get_random(&mut rng).unwrap().id;
        assert!((1..=3).contains(&sample));

        container.remove(&PersonId { id: 2 });
        assert_eq!(container.len(), 2);
        let sample = container.get_random(&mut rng).unwrap().id;
        assert!((sample == 1) | (sample == 3));

        container.insert(PersonId { id: 0 });
        assert_eq!(container.len(), 3);
        let sample = container.get_random(&mut rng).unwrap().id;
        assert!((sample <= 1) | (sample == 3));

        container.remove(&PersonId { id: 0 });
        assert_eq!(container.len(), 2);
        let sample = container.get_random(&mut rng).unwrap().id;
        assert!((sample == 1) | (sample == 3));

        container.remove(&PersonId { id: 3 });
        assert_eq!(container.len(), 1);
        let sample = container.get_random(&mut rng).unwrap().id;
        assert_eq!(sample, 1);

        container.remove(&PersonId { id: 1 });
        assert_eq!(container.len(), 0);
        assert!(container.get_random(&mut rng).is_none());
    }
}
