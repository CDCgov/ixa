//! A priority queue that stores arbitrary data sorted by time
//!
//! Defines a `Queue<T>` that is intended to store a queue of items of type T,
//! sorted by `f64` time, called 'plans'. This queue has methods for adding
//! plans, cancelling plans, and retrieving the earliest plan in the queue.
//! Adding a plan is *O*(log(*n*)) while cancellation and retrieval are *O*(1).
//!
//! This queue is used by `Context` to store future events where some callback
//! closure `FnOnce(&mut Context)` will be executed at a given point in time.

use std::{
    cmp::Ordering,
    collections::{BinaryHeap, HashMap},
};

/// A priority queue that stores arbitrary data sorted by time
///
/// Items of type `T` are stored in order by `f64` time and called `Plan<T>`.
/// When plans are created they are sequentially assigned an `Id` that is a
/// wrapped `u64`. If two plans are scheduled for the same time the plan that is
/// scheduled first (i.e., that has the lowest id) is placed earlier.
///
/// The pair of time and plan id are stored in a binary heap of `Entry` objects.
/// The data payload of the event is stored in a hash map by plan id.
/// Plan cancellation occurs by removing the corresponding entry from the data
/// hash map.
pub struct Queue<T> {
    queue: BinaryHeap<Entry>,
    data_map: HashMap<u64, T>,
    plan_counter: u64,
}

impl<T> Queue<T> {
    /// Create a new empty `Queue<T>`
    #[must_use]
    pub fn new() -> Queue<T> {
        Queue {
            queue: BinaryHeap::new(),
            data_map: HashMap::new(),
            plan_counter: 0,
        }
    }

    /// Add a plan to the queue at the specified time
    ///
    /// Returns an `Id` for the newly-added plan that can be used to cancel it
    /// if needed.
    pub fn add_plan(&mut self, time: f64, data: T) -> Id {
        // Add plan to queue, store data, and increment counter
        let id = self.plan_counter;
        self.queue.push(Entry { time, id });
        self.data_map.insert(id, data);
        self.plan_counter += 1;
        Id { id }
    }

    /// Cancel a plan that has been added to the queue
    ///
    /// # Panics
    ///
    /// This function panics if you cancel a plan which has already
    /// been cancelled or executed.
    pub fn cancel_plan(&mut self, id: &Id) {
        // Delete the plan from the map, but leave in the queue
        // It will be skipped when the plan is popped from the queue
        self.data_map.remove(&id.id).expect("Plan does not exist");
    }

    /// Retrieve the earliest plan in the queue
    ///
    /// Returns the next plan if it exists or else `None` if the queue is empty
    pub fn get_next_plan(&mut self) -> Option<Plan<T>> {
        loop {
            // Pop from queue until we find a plan with data or queue is empty
            match self.queue.pop() {
                Some(entry) => {
                    // Skip plans that have been cancelled and thus have no data
                    if let Some(data) = self.data_map.remove(&entry.id) {
                        return Some(Plan {
                            time: entry.time,
                            data,
                        });
                    }
                }
                None => {
                    return None;
                }
            }
        }
    }
}

impl<T> Default for Queue<T> {
    fn default() -> Self {
        Self::new()
    }
}

/// A time and id pair used to order plans in the `Queue<T>`
///
/// `Entry` objects are sorted in increasing order of time and then plan id
#[derive(PartialEq, Debug)]
struct Entry {
    time: f64,
    id: u64,
}

impl Eq for Entry {}

impl PartialOrd for Entry {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// Entry objects are ordered in increasing order by time and then plan id
impl Ord for Entry {
    fn cmp(&self, other: &Self) -> Ordering {
        let time_ordering = self.time.partial_cmp(&other.time).unwrap().reverse();
        match time_ordering {
            // Break time ties in order of plan id
            Ordering::Equal => self.id.cmp(&other.id).reverse(),
            _ => time_ordering,
        }
    }
}

/// A unique identifier for a plan added to a `Queue<T>`
pub struct Id {
    id: u64,
}

/// A plan that holds data of type `T` intended to be used at the specified time
pub struct Plan<T> {
    pub time: f64,
    pub data: T,
}

#[cfg(test)]
#[allow(clippy::float_cmp)]
mod tests {
    use super::Queue;

    #[test]
    fn empty_queue() {
        let mut plan_queue = Queue::<()>::new();
        assert!(plan_queue.get_next_plan().is_none());
    }

    #[test]
    fn add_plans() {
        let mut plan_queue = Queue::new();
        plan_queue.add_plan(1.0, 1);
        plan_queue.add_plan(3.0, 3);
        plan_queue.add_plan(2.0, 2);

        let next_plan = plan_queue.get_next_plan().unwrap();
        assert_eq!(next_plan.time, 1.0);
        assert_eq!(next_plan.data, 1);

        let next_plan = plan_queue.get_next_plan().unwrap();
        assert_eq!(next_plan.time, 2.0);
        assert_eq!(next_plan.data, 2);

        let next_plan = plan_queue.get_next_plan().unwrap();
        assert_eq!(next_plan.time, 3.0);
        assert_eq!(next_plan.data, 3);

        assert!(plan_queue.get_next_plan().is_none());
    }

    #[test]
    fn add_plans_at_same_time() {
        let mut plan_queue = Queue::new();
        plan_queue.add_plan(1.0, 1);
        plan_queue.add_plan(1.0, 2);

        let next_plan = plan_queue.get_next_plan().unwrap();
        assert_eq!(next_plan.time, 1.0);
        assert_eq!(next_plan.data, 1);

        let next_plan = plan_queue.get_next_plan().unwrap();
        assert_eq!(next_plan.time, 1.0);
        assert_eq!(next_plan.data, 2);

        assert!(plan_queue.get_next_plan().is_none());
    }

    #[test]
    fn add_and_cancel_plans() {
        let mut plan_queue = Queue::new();
        plan_queue.add_plan(1.0, 1);
        let plan_to_cancel = plan_queue.add_plan(2.0, 2);
        plan_queue.add_plan(3.0, 3);
        plan_queue.cancel_plan(&plan_to_cancel);

        let next_plan = plan_queue.get_next_plan().unwrap();
        assert_eq!(next_plan.time, 1.0);
        assert_eq!(next_plan.data, 1);

        let next_plan = plan_queue.get_next_plan().unwrap();
        assert_eq!(next_plan.time, 3.0);
        assert_eq!(next_plan.data, 3);

        assert!(plan_queue.get_next_plan().is_none());
    }

    #[test]
    fn add_and_get_plans() {
        let mut plan_queue = Queue::new();
        plan_queue.add_plan(1.0, 1);
        plan_queue.add_plan(2.0, 2);

        let next_plan = plan_queue.get_next_plan().unwrap();
        assert_eq!(next_plan.time, 1.0);
        assert_eq!(next_plan.data, 1);

        plan_queue.add_plan(1.5, 3);

        let next_plan = plan_queue.get_next_plan().unwrap();
        assert_eq!(next_plan.time, 1.5);
        assert_eq!(next_plan.data, 3);

        let next_plan = plan_queue.get_next_plan().unwrap();
        assert_eq!(next_plan.time, 2.0);
        assert_eq!(next_plan.data, 2);

        assert!(plan_queue.get_next_plan().is_none());
    }

    #[test]
    #[should_panic(expected = "Plan does not exist")]
    fn cancel_invalid_plan() {
        let mut plan_queue = Queue::<()>::new();
        let plan_to_cancel = plan_queue.add_plan(1.0, ());
        plan_queue.get_next_plan();
        plan_queue.cancel_plan(&plan_to_cancel);
    }
}
