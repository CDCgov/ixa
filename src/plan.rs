use std::{
    cmp::Ordering,
    collections::{BinaryHeap, HashSet},
};

use derivative::Derivative;

use crate::context::Context;

pub struct PlanId {
    id: u64,
}

#[derive(Derivative)]
#[derivative(Eq, PartialEq, Debug)]
pub struct TimedPlan {
    pub time: f64,
    plan_id: u64,
    #[derivative(PartialEq = "ignore", Debug = "ignore")]
    pub callback: Box<dyn FnOnce(&mut Context)>,
}

impl Ord for TimedPlan {
    fn cmp(&self, other: &Self) -> Ordering {
        let time_ordering = self.time.partial_cmp(&other.time).unwrap().reverse();
        if time_ordering == Ordering::Equal {
            // Break time ties in order of plan id
            self.plan_id.cmp(&other.plan_id).reverse()
        } else {
            time_ordering
        }
    }
}

impl PartialOrd for TimedPlan {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

#[derive(Debug)]
pub struct PlanQueue {
    queue: BinaryHeap<TimedPlan>,
    invalid_set: HashSet<u64>,
    plan_counter: u64,
}

impl Default for PlanQueue {
    fn default() -> Self {
        Self::new()
    }
}

impl PlanQueue {
    pub fn new() -> PlanQueue {
        PlanQueue {
            queue: BinaryHeap::new(),
            invalid_set: HashSet::new(),
            plan_counter: 0,
        }
    }

    pub fn add_plan(&mut self, time: f64, callback: impl FnOnce(&mut Context) + 'static) -> PlanId {
        // Add plan to queue and increment counter
        let plan_id = self.plan_counter;
        self.queue.push(TimedPlan {
            time,
            plan_id,
            callback: Box::new(callback),
        });
        self.plan_counter += 1;
        PlanId { id: plan_id }
    }

    pub fn cancel_plan(&mut self, id: PlanId) {
        self.invalid_set.insert(id.id);
    }

    pub fn get_next_timed_plan(&mut self) -> Option<TimedPlan> {
        loop {
            let next_timed_plan = self.queue.pop();
            match next_timed_plan {
                Some(timed_plan) => {
                    if self.invalid_set.contains(&timed_plan.plan_id) {
                        self.invalid_set.remove(&timed_plan.plan_id);
                    } else {
                        return Some(timed_plan);
                    }
                }
                None => {
                    return None;
                }
            }
        }
    }
}
