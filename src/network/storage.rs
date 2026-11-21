/*!

`LayerStorage<E, D>` holds the adjacency lists of one layer: a `Vec<Edge<E, D>>` per
entity, indexed by `EntityId<E>` and grown as needed, like a property column. It
enforces one edge per (source, neighbor) pair and nothing else; the rest of the rules
live in the public API.

A [`Batch`] appends many edges at once: all of them or none, with the uniqueness check
of a long list done once at the end instead of once per edge.

*/

use crate::entity::{Entity, EntityId};
use crate::network::edge::Edge;
use crate::IxaError;

/// A batch scans a list for a duplicate on each append up to this length, and checks a
/// longer list once at the end. Scanning per append is quadratic in the length of a list,
/// but cheaper than the bookkeeping of a deferred check while the list is short.
const DEFERRED_CHECK_LENGTH: usize = 32;

pub(super) struct LayerStorage<E: Entity, D> {
    lists: Vec<Vec<Edge<E, D>>>,
    /// The total length of `lists`, kept in step by everything that adds or removes an edge.
    edge_count: usize,
}

impl<E: Entity, D> Default for LayerStorage<E, D> {
    fn default() -> Self {
        Self {
            lists: Vec::new(),
            edge_count: 0,
        }
    }
}

impl<E: Entity, D> LayerStorage<E, D> {
    /// Inserts `edge` into the list of `from`, or fails if `from` already has an edge
    /// to the same neighbor.
    pub fn add_edge(&mut self, from: EntityId<E>, edge: Edge<E, D>) -> Result<(), IxaError> {
        if self.position(from, edge.neighbor).is_some() {
            return Err(IxaError::EdgeAlreadyExists);
        }
        self.push_unchecked(from, edge);
        Ok(())
    }

    /// Appends `edge` to the list of `from` without the uniqueness check. The caller
    /// must have checked.
    pub fn push_unchecked(&mut self, from: EntityId<E>, edge: Edge<E, D>) {
        self.append(from, edge);
        self.edge_count += 1;
    }

    /// Appends `edge` to the list of `from` and leaves `edge_count` to the caller.
    fn append(&mut self, from: EntityId<E>, edge: Edge<E, D>) {
        let index = from.0;
        if index >= self.lists.len() {
            self.lists.resize_with(index + 1, Vec::new);
        }
        self.lists[index].push(edge);
    }

    /// Starts a batch expected to append about `expected` edges.
    pub fn batch(&mut self, expected: usize) -> Batch<'_, E, D> {
        Batch {
            storage: self,
            appended: Vec::with_capacity(expected),
            unchecked: Vec::new(),
        }
    }

    /// Removes and returns the edge from `from` to `to`, if any. The order of the
    /// remaining edges of `from` is not preserved.
    pub fn remove_edge(&mut self, from: EntityId<E>, to: EntityId<E>) -> Option<Edge<E, D>> {
        let pos = self.position(from, to)?;
        Some(self.remove_at(from, pos))
    }

    /// The index of the edge from `from` to `to` in the list of `from`, if any.
    pub fn position(&self, from: EntityId<E>, to: EntityId<E>) -> Option<usize> {
        self.get_edges(from)
            .iter()
            .position(|edge| edge.neighbor == to)
    }

    /// Swap-removes the edge at `pos`, a position from [`position`](Self::position).
    pub fn remove_at(&mut self, from: EntityId<E>, pos: usize) -> Edge<E, D> {
        self.edge_count -= 1;
        self.lists[from.0].swap_remove(pos)
    }

    pub fn get_edge(&self, from: EntityId<E>, to: EntityId<E>) -> Option<&Edge<E, D>> {
        self.get_edges(from).iter().find(|edge| edge.neighbor == to)
    }

    pub fn get_edge_mut(&mut self, from: EntityId<E>, to: EntityId<E>) -> Option<&mut Edge<E, D>> {
        self.lists
            .get_mut(from.0)?
            .iter_mut()
            .find(|edge| edge.neighbor == to)
    }

    /// The outgoing edges of `from`, empty if it has none.
    pub fn get_edges(&self, from: EntityId<E>) -> &[Edge<E, D>] {
        self.lists.get(from.0).map_or(&[], Vec::as_slice)
    }

    /// The number of edges, counting each direction.
    pub fn edge_count(&self) -> usize {
        self.edge_count
    }

    /// Every edge with its source, by source id and then in list order.
    pub fn iter(&self) -> impl Iterator<Item = (EntityId<E>, &Edge<E, D>)> {
        self.lists.iter().enumerate().flat_map(|(index, list)| {
            let source = EntityId::new(index);
            list.iter().map(move |edge| (source, edge))
        })
    }

    /// Entities among ids `0..population` with exactly `degree` outgoing edges.
    pub fn find_entities_by_degree(&self, degree: usize, population: usize) -> Vec<EntityId<E>> {
        (0..population)
            .map(EntityId::new)
            .filter(|&id| self.get_edges(id).len() == degree)
            .collect()
    }

    /// Counts of entities by out-degree over ids `0..population`: `result[d]` is the
    /// number of entities with degree `d`, so the result has `max degree + 1` entries,
    /// at least one.
    pub fn degree_distribution(&self, population: usize) -> Vec<usize> {
        let mut distribution = vec![0];
        let mut connected = 0;
        for list in self.lists.iter().take(population) {
            let degree = list.len();
            // Entities without edges are counted by subtraction: most of a sparse layer.
            if degree == 0 {
                continue;
            }
            connected += 1;
            if degree >= distribution.len() {
                distribution.resize(degree + 1, 0);
            }
            distribution[degree] += 1;
        }
        distribution[0] = population - connected;
        distribution
    }

    /// Removes every edge. Each entity's list keeps its allocation, so re-adding the
    /// edges later (reopening a closed setting) does not reallocate.
    pub fn clear(&mut self) {
        for list in &mut self.lists {
            list.clear();
        }
        self.edge_count = 0;
    }
}

/// Edges being appended to a layer together. Dropping the batch removes them again, so an
/// error or a panic part way through leaves the layer as it was; [`commit`](Self::commit)
/// keeps them.
pub(super) struct Batch<'a, E: Entity, D> {
    storage: &'a mut LayerStorage<E, D>,
    /// The source of every edge appended so far, one entry per edge.
    appended: Vec<usize>,
    /// The source of every edge appended without a uniqueness check, one entry per edge.
    unchecked: Vec<usize>,
}

impl<E: Entity, D> Batch<'_, E, D> {
    /// Appends `edge` to the list of `from`. A duplicate in a short list fails here; one
    /// in a long list fails in [`commit`](Self::commit).
    pub fn push(&mut self, from: EntityId<E>, edge: Edge<E, D>) -> Result<(), IxaError> {
        let length = self.storage.get_edges(from).len();
        if length < DEFERRED_CHECK_LENGTH {
            if self.storage.position(from, edge.neighbor).is_some() {
                return Err(IxaError::EdgeAlreadyExists);
            }
        } else {
            self.unchecked.push(from.0);
        }
        self.storage.append(from, edge);
        self.appended.push(from.0);
        Ok(())
    }

    /// Keeps the appended edges if no list ends up with two edges to one neighbor, and
    /// removes them all otherwise.
    pub fn commit(mut self) -> Result<(), IxaError> {
        self.unchecked.sort_unstable();
        let mut neighbors = Vec::new();
        for run in self.unchecked.chunk_by(|left, right| left == right) {
            let list = &self.storage.lists[run[0]];
            if tail_has_duplicate(list, run.len(), &mut neighbors) {
                return Err(IxaError::EdgeAlreadyExists);
            }
        }
        // Counted once here, not per edge: nothing can read the count during a batch.
        self.storage.edge_count += self.appended.len();
        self.appended.clear();
        Ok(())
    }
}

impl<E: Entity, D> Drop for Batch<'_, E, D> {
    fn drop(&mut self) {
        // Every entry stands for one edge at the end of its source's list.
        for &source in &self.appended {
            self.storage.lists[source].pop();
        }
    }
}

/// Whether any of the last `unchecked` edges of `list` repeats the neighbor of an earlier
/// edge. The edges before them are known to be distinct. `neighbors` is scratch space.
fn tail_has_duplicate<E: Entity, D>(
    list: &[Edge<E, D>],
    unchecked: usize,
    neighbors: &mut Vec<usize>,
) -> bool {
    // A few new edges are cheaper to look up one by one than sorting the whole list.
    if unchecked <= list.len().ilog2() as usize {
        for position in (list.len() - unchecked)..list.len() {
            let neighbor = list[position].neighbor;
            if list[..position]
                .iter()
                .any(|edge| edge.neighbor == neighbor)
            {
                return true;
            }
        }
        return false;
    }
    neighbors.clear();
    neighbors.extend(list.iter().map(|edge| edge.neighbor.0));
    neighbors.sort_unstable();
    neighbors.windows(2).any(|pair| pair[0] == pair[1])
}

#[cfg(test)]
mod tests {
    use super::{LayerStorage, DEFERRED_CHECK_LENGTH};
    use crate::network::edge::Edge;
    use crate::{define_entity, IxaError};

    define_entity!(Person);

    fn edge(neighbor: usize) -> Edge<Person, ()> {
        Edge {
            neighbor: PersonId::new(neighbor),
            data: (),
        }
    }

    /// Entity 0 with an edge to each of `1..=degree`.
    fn hub(degree: usize) -> LayerStorage<Person, ()> {
        let mut storage = LayerStorage::default();
        let mut batch = storage.batch(degree);
        for neighbor in 1..=degree {
            batch.push(PersonId::new(0), edge(neighbor)).unwrap();
        }
        batch.commit().unwrap();
        storage
    }

    fn add_to_hub(storage: &mut LayerStorage<Person, ()>, neighbors: &[usize]) -> bool {
        let mut batch = storage.batch(neighbors.len());
        for &neighbor in neighbors {
            match batch.push(PersonId::new(0), edge(neighbor)) {
                Ok(()) => {}
                Err(IxaError::EdgeAlreadyExists) => return false,
                Err(error) => panic!("unexpected error {error}"),
            }
        }
        batch.commit().is_ok()
    }

    #[test]
    fn batch_keeps_distinct_edges_of_a_long_list() {
        let degree = DEFERRED_CHECK_LENGTH * 4;
        let storage = hub(degree);
        let neighbors: Vec<usize> = storage
            .get_edges(PersonId::new(0))
            .iter()
            .map(|edge| edge.neighbor.0)
            .collect();
        assert_eq!(neighbors, (1..=degree).collect::<Vec<_>>());
    }

    #[test]
    fn batch_rejects_a_duplicate_in_a_long_list() {
        let degree = DEFERRED_CHECK_LENGTH * 4;
        let mut storage = hub(degree);
        let many_new: Vec<usize> = (degree + 1..=degree * 2).collect();

        // One new edge is looked up, many are checked by sorting: both must catch a
        // repeat of an existing neighbor and a repeat within the batch.
        let mut repeats_existing = many_new.clone();
        repeats_existing.push(degree / 2);
        let mut repeats_itself = many_new.clone();
        repeats_itself.push(degree + 1);
        for rejected in [vec![degree], repeats_existing, repeats_itself] {
            assert!(!add_to_hub(&mut storage, &rejected));
            assert_eq!(storage.get_edges(PersonId::new(0)).len(), degree);
        }

        assert!(add_to_hub(&mut storage, &[degree * 2 + 1]));
        assert!(add_to_hub(&mut storage, &many_new));
        assert_eq!(storage.get_edges(PersonId::new(0)).len(), degree * 2 + 1);
    }

    #[test]
    fn edge_count_matches_the_lists_through_mixed_changes() {
        let mut storage: LayerStorage<Person, ()> = LayerStorage::default();
        let population = 40;
        let mut state: u64 = 0x9E37_79B9_7F4A_7C15;
        let mut next = |bound: usize| {
            state = state
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1);
            (state >> 33) as usize % bound
        };
        for step in 0..5_000 {
            let from = PersonId::new(next(population));
            let to = next(population);
            match next(10) {
                0..=4 => {
                    let _ = storage.add_edge(from, edge(to));
                }
                5..=6 => {
                    let _ = storage.remove_edge(from, PersonId::new(to));
                }
                7..=8 => {
                    // A batch of a few edges, some of which repeat and roll it back.
                    let mut batch = storage.batch(4);
                    let mut accepted = true;
                    for _ in 0..4 {
                        let neighbor = next(population);
                        accepted = accepted && batch.push(from, edge(neighbor)).is_ok();
                    }
                    if accepted {
                        let _ = batch.commit();
                    }
                }
                _ => {
                    if step % 500 == 0 {
                        storage.clear();
                    }
                }
            }
            let total: usize = storage.lists.iter().map(Vec::len).sum();
            assert_eq!(storage.edge_count(), total);
            assert_eq!(storage.iter().count(), total);
        }
        assert!(storage.edge_count() > 0);
    }

    #[test]
    fn dropped_batch_removes_its_edges_from_every_list() {
        let mut storage = hub(DEFERRED_CHECK_LENGTH * 2);
        let mut batch = storage.batch(3);
        batch.push(PersonId::new(0), edge(1000)).unwrap();
        batch.push(PersonId::new(5), edge(0)).unwrap();
        batch.push(PersonId::new(0), edge(1001)).unwrap();
        drop(batch);
        assert_eq!(
            storage.get_edges(PersonId::new(0)).len(),
            DEFERRED_CHECK_LENGTH * 2
        );
        assert!(storage.get_edges(PersonId::new(5)).is_empty());
    }
}
