use std::{
    cmp::Ordering,
    collections::{BinaryHeap, HashMap},
};

pub struct PlanId {
    id: usize,
}

pub struct Plan<T> {
    pub time: f64,
    pub data: T,
}

#[derive(PartialEq, Debug)]
pub struct PlanRecord {
    pub time: f64,
    id: usize,
}

impl Eq for PlanRecord {}

impl PartialOrd for PlanRecord {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for PlanRecord {
    fn cmp(&self, other: &Self) -> Ordering {
        let time_ordering = self.time.partial_cmp(&other.time).unwrap().reverse();
        if time_ordering == Ordering::Equal {
            // Break time ties in order of plan id
            self.id.cmp(&other.id).reverse()
        } else {
            time_ordering
        }
    }
}

#[derive(Debug)]
pub struct PlanQueue<T> {
    queue: BinaryHeap<PlanRecord>,
    data_map: HashMap<usize, T>,
    plan_counter: usize,
}

impl<T> Default for PlanQueue<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T> PlanQueue<T> {
    pub fn new() -> PlanQueue<T> {
        PlanQueue {
            queue: BinaryHeap::new(),
            data_map: HashMap::new(),
            plan_counter: 0,
        }
    }

    pub fn add_plan(&mut self, time: f64, data: T) -> PlanId {
        // Add plan to queue, store data, and increment counter
        let id = self.plan_counter;
        self.queue.push(PlanRecord { time, id });
        self.data_map.insert(id, data);
        self.plan_counter += 1;
        PlanId { id }
    }

    pub fn cancel_plan(&mut self, id: PlanId) {
        self.data_map.remove(&id.id).expect("Plan does not exist");
    }

    pub fn get_next_plan(&mut self) -> Option<Plan<T>> {
        loop {
            match self.queue.pop() {
                Some(plan_record) => {
                    if let Some(data) = self.data_map.remove(&plan_record.id) {
                        return Some(Plan {
                            time: plan_record.time,
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

#[cfg(test)]
mod tests {
    use super::PlanQueue;

    #[test]
    fn test_add_cancel() {
        // Add some plans and cancel and make sure ordering occurs as expected
        let mut plan_queue = PlanQueue::<usize>::new();
        plan_queue.add_plan(1.0, 1);
        plan_queue.add_plan(3.0, 3);
        plan_queue.add_plan(3.0, 4);
        let plan_to_cancel = plan_queue.add_plan(1.5, 0);
        plan_queue.add_plan(2.0, 2);
        plan_queue.cancel_plan(plan_to_cancel);

        assert_eq!(plan_queue.get_next_plan().unwrap().time, 1.0);
        assert_eq!(plan_queue.get_next_plan().unwrap().time, 2.0);

        // Check tie handling
        let next_plan = plan_queue.get_next_plan().unwrap();
        assert_eq!(next_plan.time, 3.0);
        assert_eq!(next_plan.data, 3);

        let next_plan = plan_queue.get_next_plan().unwrap();
        assert_eq!(next_plan.time, 3.0);
        assert_eq!(next_plan.data, 4);

        assert!(plan_queue.get_next_plan().is_none());
    }

    #[test]
    #[should_panic]
    fn test_invalid_cancel() {
        // Cancel a plan that has already occured and make sure it panics
        let mut plan_queue = PlanQueue::<()>::new();
        let plan_to_cancel = plan_queue.add_plan(1.0, ());
        plan_queue.get_next_plan();
        plan_queue.cancel_plan(plan_to_cancel);
    }
}
