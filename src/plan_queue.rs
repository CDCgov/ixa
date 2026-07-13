//! Implementation details for Ixa's plan queues.
//!
//! [`PlanId`] is the only public item in this module. The queues store regular
//! time-ordered plans, startup-time plans, and shutdown-time plans, sharing a
//! single `PlanId` allocator and cancellation map.

use std::cmp::Ordering;
use std::collections::BinaryHeap;

use crate::context::{Context, ExecutionPhase};
use crate::{trace, HashMap, HashMapExt};

type Callback = dyn FnOnce(&mut Context);
type BoxedCallback = Box<Callback>;

struct QueuedPlan {
    callback: BoxedCallback,
    is_passive: bool,
}

#[derive(Clone, Copy)]
enum DistinguishedQueue {
    Startup,
    Shutdown,
}

/// A priority queue that stores scheduled plans.
///
/// Regular plans are ordered by simulation time, execution phase, and plan ID.
/// Startup-time and shutdown-time plans are ordered by execution phase and plan
/// ID; their stored time is only an internal constant and has no simulation-time
/// meaning.
pub(crate) struct PlanQueue {
    queue: BinaryHeap<PlanSchedule>,
    startup_queue: BinaryHeap<PlanSchedule>,
    shutdown_queue: BinaryHeap<PlanSchedule>,
    data_map: HashMap<u64, QueuedPlan>,
    /// Count of scheduled plans excluding passive, startup-time, and shutdown-time plans.
    regular_plan_count: usize,
    /// The next plan ID that will be issued.
    next_plan_id: u64,
    /// Tracks the high water mark of plans in flight (scheduled but not yet executed).
    /// This is calculated from the combined lengths of the three heaps, not
    /// from `self.data_map.len()`.
    #[cfg(feature = "profiling")]
    pub(crate) max_plans_in_flight: u64,
    #[cfg(feature = "profiling")]
    pub(crate) max_memory_in_use: u64,
}

impl PlanQueue {
    /// Create a new empty `PlanQueue`.
    #[must_use]
    pub(crate) fn new() -> PlanQueue {
        PlanQueue {
            queue: BinaryHeap::new(),
            startup_queue: BinaryHeap::new(),
            shutdown_queue: BinaryHeap::new(),
            data_map: HashMap::new(),
            regular_plan_count: 0,
            next_plan_id: 0,
            #[cfg(feature = "profiling")]
            max_plans_in_flight: 0,
            #[cfg(feature = "profiling")]
            max_memory_in_use: 0,
        }
    }

    /// Add a regular plan to the queue at the specified time.
    ///
    /// Returns a [`PlanId`] for the newly-added plan that can be used to cancel it
    /// if needed.
    pub(crate) fn add_plan(
        &mut self,
        time: f64,
        callback: BoxedCallback,
        phase: ExecutionPhase,
        is_passive: bool,
    ) -> PlanId {
        trace!("adding plan at {time}");
        let plan_id = self.next_plan_id;
        self.queue.push(PlanSchedule {
            plan_id,
            time,
            phase,
        });
        self.data_map.insert(
            plan_id,
            QueuedPlan {
                callback,
                is_passive,
            },
        );
        if !is_passive {
            self.regular_plan_count += 1;
        }
        self.next_plan_id += 1;
        self.update_profiling_high_water_marks();

        PlanId(plan_id)
    }

    /// Add a startup-time plan.
    ///
    /// Startup-time plans have no simulation time. They are ordered by phase and
    /// plan ID.
    pub(crate) fn add_startup_plan(
        &mut self,
        callback: BoxedCallback,
        phase: ExecutionPhase,
    ) -> PlanId {
        trace!("adding startup-time plan");
        self.add_distinguished_plan(callback, phase, DistinguishedQueue::Startup)
    }

    /// Add a shutdown-time plan.
    ///
    /// Shutdown-time plans have no simulation time. They are ordered by phase and
    /// plan ID.
    pub(crate) fn add_shutdown_plan(
        &mut self,
        callback: BoxedCallback,
        phase: ExecutionPhase,
    ) -> PlanId {
        trace!("adding shutdown-time plan");
        self.add_distinguished_plan(callback, phase, DistinguishedQueue::Shutdown)
    }

    fn add_distinguished_plan(
        &mut self,
        callback: BoxedCallback,
        phase: ExecutionPhase,
        queue: DistinguishedQueue,
    ) -> PlanId {
        let plan_id = self.next_plan_id;
        let schedule = PlanSchedule {
            plan_id,
            time: 0.0,
            phase,
        };
        match queue {
            DistinguishedQueue::Startup => self.startup_queue.push(schedule),
            DistinguishedQueue::Shutdown => self.shutdown_queue.push(schedule),
        }
        self.data_map.insert(
            plan_id,
            QueuedPlan {
                callback,
                is_passive: true,
            },
        );
        self.next_plan_id += 1;
        self.update_profiling_high_water_marks();

        PlanId(plan_id)
    }

    /// Cancel a plan that has been added to any queue.
    pub(crate) fn cancel_plan(&mut self, plan_id: &PlanId) -> Option<BoxedCallback> {
        trace!("cancel plan {plan_id:?}");
        // Delete the plan from the map, but leave in the heap. It will be skipped
        // when its heap entry reaches the root.
        self.data_map.remove(&plan_id.0).map(|queued_plan| {
            if !queued_plan.is_passive {
                self.regular_plan_count -= 1;
            }
            queued_plan.callback
        })
    }

    /// Return the time the next plan is scheduled for, if there is one.
    #[must_use]
    pub(crate) fn next_time(&mut self) -> Option<f64> {
        while let Some(entry) = self.queue.peek() {
            // We only want to report the time if the plan has not been canceled.
            if self.data_map.contains_key(&entry.plan_id) {
                return Some(entry.time);
            }
            // Trim the canceled plan.
            self.queue.pop();
        }
        None
    }

    /// Completely empties every queue, including startup-time and shutdown-time plans.
    #[allow(dead_code)]
    pub(crate) fn clear(&mut self) {
        self.data_map.clear();
        self.queue.clear();
        self.startup_queue.clear();
        self.shutdown_queue.clear();
        self.regular_plan_count = 0;
        self.next_plan_id = 0;
    }

    /// Retrieve the earliest regular plan only if the regular queue is active.
    ///
    /// The queue is active while at least one non-passive regular plan is
    /// scheduled. The returned plan is simply the next regular plan by time,
    /// phase, and plan ID; it may be passive. If no live non-passive regular
    /// plan is scheduled, this returns `None` without removing passive plans
    /// from the regular queue.
    pub(crate) fn pop_next_if_active(&mut self) -> Option<Plan> {
        if self.regular_plan_count == 0 {
            return None;
        }

        trace!("getting next plan");
        loop {
            // The `pop` should be infallible when the plan count is positive unless the
            // queue invariants have been violated.
            let entry = self
                .queue
                .pop()
                .expect("plan count was positive but no plan was available");

            // Discard any cancelled plans we encounter.
            if let Some(queued_plan) = self.data_map.remove(&entry.plan_id) {
                if !queued_plan.is_passive {
                    self.regular_plan_count -= 1;
                }
                return Some(Plan {
                    time: entry.time,
                    data: queued_plan.callback,
                });
            }
        }
    }

    /// Retrieve the earliest regular plan only if it is scheduled at `time`.
    ///
    /// Returns `None` without removing a future plan if the next regular plan is
    /// later than `time`.
    pub(crate) fn pop_next_at(&mut self, time: f64) -> Option<Plan> {
        loop {
            match self.queue.peek() {
                // Trim any cancelled plans
                Some(entry) if !self.data_map.contains_key(&entry.plan_id) => {
                    self.queue.pop();
                }

                // Return only if the plan is scheduled for the given time
                Some(entry) if entry.time == time => {
                    // Pop is infallible here.
                    let entry = self.queue.pop().unwrap();
                    let queued_plan = self
                        .data_map
                        .remove(&entry.plan_id)
                        .expect("live plan must have callback");
                    if !queued_plan.is_passive {
                        self.regular_plan_count -= 1;
                    }
                    return Some(Plan {
                        time,
                        data: queued_plan.callback,
                    });
                }

                // There are no plans scheduled at the given time
                _ => return None,
            }
        }
    }

    /// Retrieve the next startup-time plan.
    ///
    /// Returns the next startup-time plan if it exists or else `None` if the
    /// startup-time queue is empty.
    pub(crate) fn pop_next_startup(&mut self) -> Option<Plan> {
        trace!("getting next startup-time plan");
        self.pop_next_distinguished(DistinguishedQueue::Startup)
    }

    /// Retrieve the next shutdown-time plan.
    ///
    /// Returns the next shutdown-time plan if it exists or else `None` if the
    /// shutdown-time queue is empty.
    pub(crate) fn pop_next_shutdown(&mut self) -> Option<Plan> {
        trace!("getting next shutdown-time plan");
        self.pop_next_distinguished(DistinguishedQueue::Shutdown)
    }

    fn pop_next_distinguished(&mut self, queue: DistinguishedQueue) -> Option<Plan> {
        loop {
            let entry = match queue {
                DistinguishedQueue::Startup => self.startup_queue.pop(),
                DistinguishedQueue::Shutdown => self.shutdown_queue.pop(),
            }?;

            // If there's no `data_map` entry, the plan has been canceled, so discard
            // and pop another plan.
            let Some(queued_plan) = self.data_map.remove(&entry.plan_id) else {
                continue;
            };
            if !queued_plan.is_passive {
                self.regular_plan_count -= 1;
            }
            return Some(Plan {
                time: entry.time,
                data: queued_plan.callback,
            });
        }
    }

    fn update_profiling_high_water_marks(&mut self) {
        #[cfg(feature = "profiling")]
        {
            let plans_in_flight =
                self.queue.len() + self.startup_queue.len() + self.shutdown_queue.len();
            self.max_plans_in_flight = self.max_plans_in_flight.max(plans_in_flight as u64);
            self.max_memory_in_use = self
                .max_memory_in_use
                .max(self.estimated_memory_in_use() as u64);
        }
    }

    #[cfg(feature = "profiling")]
    fn estimated_memory_in_use(&self) -> usize {
        let queue_bytes = (self.queue.capacity()
            + self.startup_queue.capacity()
            + self.shutdown_queue.capacity())
            * size_of::<PlanSchedule>();

        let map_entry_bytes = self.data_map.capacity() * size_of::<(u64, QueuedPlan)>();

        queue_bytes + map_entry_bytes
    }
}

impl Default for PlanQueue {
    fn default() -> Self {
        Self::new()
    }
}

/// A time, id, and phase object used to order plans in a [`PlanQueue`].
///
/// Regular [`PlanSchedule`] objects are sorted in increasing order of time,
/// phase, and then plan id. Startup-time and shutdown-time schedules all have
/// the same internal time and are therefore sorted by phase and then plan id.
#[derive(PartialEq, Debug, Clone, Copy)]
pub(crate) struct PlanSchedule {
    pub plan_id: u64,
    pub time: f64,
    pub phase: ExecutionPhase,
}

impl Eq for PlanSchedule {}

impl PartialOrd for PlanSchedule {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// Entry objects are ordered in increasing order by time, phase, and then plan id.
impl Ord for PlanSchedule {
    fn cmp(&self, other: &Self) -> Ordering {
        let time_ordering = self.time.partial_cmp(&other.time).unwrap().reverse();
        match time_ordering {
            Ordering::Equal => {
                let phase_ordering = self.phase.partial_cmp(&other.phase).unwrap().reverse();
                match phase_ordering {
                    Ordering::Equal => self.plan_id.cmp(&other.plan_id).reverse(),
                    _ => phase_ordering,
                }
            }
            _ => time_ordering,
        }
    }
}

/// A unique identifier for a plan scheduled on a [`Context`].
///
/// `PlanId` values are returned by [`Context::add_plan`] and related scheduling
/// methods, and can be passed to [`Context::cancel_plan`].
///
/// # Examples
///
/// ```
/// use ixa::{Context, PlanId};
///
/// let mut context = Context::new();
/// let plan_id: PlanId = context.add_plan(1.0, |_| {});
/// context.cancel_plan(&plan_id);
/// ```
#[derive(Clone, Copy, Debug, Hash, Eq, PartialEq)]
pub struct PlanId(pub(crate) u64);

/// A plan that holds a callback intended to be executed at the specified time.
pub(crate) struct Plan {
    pub time: f64,
    pub data: BoxedCallback,
}

#[cfg(test)]
#[allow(clippy::float_cmp)]
mod tests {
    use std::cell::RefCell;
    use std::rc::Rc;

    use super::PlanQueue;
    use crate::context::{Context, ExecutionPhase};

    fn callback(value: u32, observed: Rc<RefCell<Vec<u32>>>) -> Box<dyn FnOnce(&mut Context)> {
        Box::new(move |_| observed.borrow_mut().push(value))
    }

    fn run_plan(plan: super::Plan, context: &mut Context) {
        (plan.data)(context);
    }

    #[test]
    fn empty_queue() {
        let mut plan_queue = PlanQueue::new();
        assert!(plan_queue.pop_next_if_active().is_none());
    }

    #[test]
    fn add_plans() {
        let observed = Rc::new(RefCell::new(Vec::new()));
        let mut context = Context::new();
        let mut plan_queue = PlanQueue::new();
        plan_queue.add_plan(
            1.0,
            callback(1, Rc::clone(&observed)),
            ExecutionPhase::Normal,
            false,
        );
        plan_queue.add_plan(
            3.0,
            callback(3, Rc::clone(&observed)),
            ExecutionPhase::Normal,
            false,
        );
        plan_queue.add_plan(
            2.0,
            callback(2, Rc::clone(&observed)),
            ExecutionPhase::Normal,
            false,
        );
        assert_eq!(plan_queue.next_time(), Some(1.0));

        let next_plan = plan_queue.pop_next_if_active().unwrap();
        assert_eq!(next_plan.time, 1.0);
        run_plan(next_plan, &mut context);

        assert_eq!(plan_queue.next_time(), Some(2.0));
        let next_plan = plan_queue.pop_next_if_active().unwrap();
        assert_eq!(next_plan.time, 2.0);
        run_plan(next_plan, &mut context);

        assert_eq!(plan_queue.next_time(), Some(3.0));
        let next_plan = plan_queue.pop_next_if_active().unwrap();
        assert_eq!(next_plan.time, 3.0);
        run_plan(next_plan, &mut context);

        assert!(plan_queue.pop_next_if_active().is_none());
        assert_eq!(*observed.borrow(), vec![1, 2, 3]);
    }

    #[test]
    fn add_plans_at_same_time_with_same_phase() {
        let observed = Rc::new(RefCell::new(Vec::new()));
        let mut context = Context::new();
        let mut plan_queue = PlanQueue::new();
        plan_queue.add_plan(
            1.0,
            callback(1, Rc::clone(&observed)),
            ExecutionPhase::Normal,
            false,
        );
        plan_queue.add_plan(
            1.0,
            callback(2, Rc::clone(&observed)),
            ExecutionPhase::Normal,
            false,
        );

        let next_plan = plan_queue.pop_next_if_active().unwrap();
        assert_eq!(next_plan.time, 1.0);
        run_plan(next_plan, &mut context);
        let next_plan = plan_queue.pop_next_if_active().unwrap();
        assert_eq!(next_plan.time, 1.0);
        run_plan(next_plan, &mut context);

        assert!(plan_queue.pop_next_if_active().is_none());
        assert_eq!(*observed.borrow(), vec![1, 2]);
    }

    #[test]
    fn add_plans_at_same_time_with_different_phase() {
        let observed = Rc::new(RefCell::new(Vec::new()));
        let mut context = Context::new();
        let mut plan_queue = PlanQueue::new();
        plan_queue.add_plan(
            1.0,
            callback(1, Rc::clone(&observed)),
            ExecutionPhase::Normal,
            false,
        );
        plan_queue.add_plan(
            1.0,
            callback(2, Rc::clone(&observed)),
            ExecutionPhase::First,
            false,
        );

        let next_plan = plan_queue.pop_next_if_active().unwrap();
        assert_eq!(next_plan.time, 1.0);
        run_plan(next_plan, &mut context);
        let next_plan = plan_queue.pop_next_if_active().unwrap();
        assert_eq!(next_plan.time, 1.0);
        run_plan(next_plan, &mut context);

        assert!(plan_queue.pop_next_if_active().is_none());
        assert_eq!(*observed.borrow(), vec![2, 1]);
    }

    #[test]
    fn cancel_plan() {
        let observed = Rc::new(RefCell::new(Vec::new()));
        let mut context = Context::new();
        let mut plan_queue = PlanQueue::new();
        plan_queue.add_plan(
            1.0,
            callback(1, Rc::clone(&observed)),
            ExecutionPhase::Normal,
            false,
        );
        let plan_to_cancel = plan_queue.add_plan(
            2.0,
            callback(2, Rc::clone(&observed)),
            ExecutionPhase::Normal,
            false,
        );
        plan_queue.add_plan(
            3.0,
            callback(3, Rc::clone(&observed)),
            ExecutionPhase::Normal,
            false,
        );
        plan_queue.cancel_plan(&plan_to_cancel);

        let next_plan = plan_queue.pop_next_if_active().unwrap();
        assert_eq!(next_plan.time, 1.0);
        run_plan(next_plan, &mut context);

        let next_plan = plan_queue.pop_next_if_active().unwrap();
        assert_eq!(next_plan.time, 3.0);
        run_plan(next_plan, &mut context);

        assert!(plan_queue.pop_next_if_active().is_none());
        assert_eq!(*observed.borrow(), vec![1, 3]);
    }

    #[test]
    fn passive_only_plans_do_not_pop_during_active_execution() {
        let observed = Rc::new(RefCell::new(Vec::new()));
        let mut plan_queue = PlanQueue::new();
        plan_queue.add_plan(
            1.0,
            callback(1, Rc::clone(&observed)),
            ExecutionPhase::Normal,
            true,
        );

        assert_eq!(plan_queue.regular_plan_count, 0);
        assert!(plan_queue.pop_next_if_active().is_none());
        assert_eq!(plan_queue.next_time(), Some(1.0));
        assert!(observed.borrow().is_empty());
    }

    #[test]
    fn passive_plan_can_pop_before_later_non_passive_plan() {
        let observed = Rc::new(RefCell::new(Vec::new()));
        let mut context = Context::new();
        let mut plan_queue = PlanQueue::new();
        plan_queue.add_plan(
            1.0,
            callback(1, Rc::clone(&observed)),
            ExecutionPhase::Normal,
            true,
        );
        plan_queue.add_plan(
            2.0,
            callback(2, Rc::clone(&observed)),
            ExecutionPhase::Normal,
            false,
        );

        assert_eq!(plan_queue.regular_plan_count, 1);
        let next_plan = plan_queue.pop_next_if_active().unwrap();
        assert_eq!(next_plan.time, 1.0);
        run_plan(next_plan, &mut context);
        assert_eq!(plan_queue.regular_plan_count, 1);

        let next_plan = plan_queue.pop_next_if_active().unwrap();
        assert_eq!(next_plan.time, 2.0);
        run_plan(next_plan, &mut context);
        assert_eq!(plan_queue.regular_plan_count, 0);

        assert_eq!(*observed.borrow(), vec![1, 2]);
    }

    #[test]
    fn canceling_non_passive_plan_decrements_regular_count() {
        let observed = Rc::new(RefCell::new(Vec::new()));
        let mut plan_queue = PlanQueue::new();
        let non_passive = plan_queue.add_plan(
            1.0,
            callback(1, Rc::clone(&observed)),
            ExecutionPhase::Normal,
            false,
        );
        let passive = plan_queue.add_plan(
            2.0,
            callback(2, Rc::clone(&observed)),
            ExecutionPhase::Normal,
            true,
        );

        assert_eq!(plan_queue.regular_plan_count, 1);
        plan_queue.cancel_plan(&passive);
        assert_eq!(plan_queue.regular_plan_count, 1);
        plan_queue.cancel_plan(&non_passive);
        assert_eq!(plan_queue.regular_plan_count, 0);
    }

    #[test]
    fn shutdown_plans_do_not_affect_regular_count() {
        let observed = Rc::new(RefCell::new(Vec::new()));
        let mut context = Context::new();
        let mut plan_queue = PlanQueue::new();
        plan_queue.add_shutdown_plan(callback(1, Rc::clone(&observed)), ExecutionPhase::Normal);

        assert_eq!(plan_queue.regular_plan_count, 0);
        let next_plan = plan_queue.pop_next_shutdown().unwrap();
        run_plan(next_plan, &mut context);
        assert_eq!(plan_queue.regular_plan_count, 0);
        assert_eq!(*observed.borrow(), vec![1]);
    }

    #[test]
    fn startup_plans_do_not_affect_regular_count() {
        let observed = Rc::new(RefCell::new(Vec::new()));
        let mut context = Context::new();
        let mut plan_queue = PlanQueue::new();
        plan_queue.add_startup_plan(callback(1, Rc::clone(&observed)), ExecutionPhase::Normal);

        assert_eq!(plan_queue.regular_plan_count, 0);
        let next_plan = plan_queue.pop_next_startup().unwrap();
        run_plan(next_plan, &mut context);
        assert_eq!(plan_queue.regular_plan_count, 0);
        assert_eq!(*observed.borrow(), vec![1]);
    }

    #[test]
    fn next_time_ignores_canceled_root() {
        let observed = Rc::new(RefCell::new(Vec::new()));
        let mut plan_queue = PlanQueue::new();
        let plan_to_cancel = plan_queue.add_plan(
            1.0,
            callback(1, Rc::clone(&observed)),
            ExecutionPhase::Normal,
            false,
        );
        plan_queue.add_plan(
            2.0,
            callback(2, Rc::clone(&observed)),
            ExecutionPhase::Normal,
            false,
        );

        plan_queue.cancel_plan(&plan_to_cancel);

        assert_eq!(plan_queue.next_time(), Some(2.0));
    }

    #[test]
    fn pop_next_at_leaves_future_plan_in_queue() {
        let observed = Rc::new(RefCell::new(Vec::new()));
        let mut context = Context::new();
        let mut plan_queue = PlanQueue::new();
        plan_queue.add_plan(
            2.0,
            callback(2, Rc::clone(&observed)),
            ExecutionPhase::Normal,
            false,
        );

        assert!(plan_queue.pop_next_at(1.0).is_none());

        let next_plan = plan_queue.pop_next_if_active().unwrap();
        assert_eq!(next_plan.time, 2.0);
        run_plan(next_plan, &mut context);
        assert_eq!(*observed.borrow(), vec![2]);
    }

    #[test]
    fn shutdown_plans_use_phase_and_fifo_order() {
        let observed = Rc::new(RefCell::new(Vec::new()));
        let mut context = Context::new();
        let mut plan_queue = PlanQueue::new();
        plan_queue.add_shutdown_plan(callback(3, Rc::clone(&observed)), ExecutionPhase::Last);
        plan_queue.add_shutdown_plan(callback(1, Rc::clone(&observed)), ExecutionPhase::First);
        plan_queue.add_shutdown_plan(callback(2, Rc::clone(&observed)), ExecutionPhase::Normal);
        plan_queue.add_shutdown_plan(callback(4, Rc::clone(&observed)), ExecutionPhase::Last);

        while let Some(plan) = plan_queue.pop_next_shutdown() {
            run_plan(plan, &mut context);
        }

        assert_eq!(*observed.borrow(), vec![1, 2, 3, 4]);
    }

    #[test]
    fn startup_plans_use_phase_and_fifo_order() {
        let observed = Rc::new(RefCell::new(Vec::new()));
        let mut context = Context::new();
        let mut plan_queue = PlanQueue::new();
        plan_queue.add_startup_plan(callback(3, Rc::clone(&observed)), ExecutionPhase::Last);
        plan_queue.add_startup_plan(callback(1, Rc::clone(&observed)), ExecutionPhase::First);
        let canceled =
            plan_queue.add_startup_plan(callback(2, Rc::clone(&observed)), ExecutionPhase::Normal);
        plan_queue.add_startup_plan(callback(4, Rc::clone(&observed)), ExecutionPhase::Last);
        plan_queue.cancel_plan(&canceled);

        while let Some(plan) = plan_queue.pop_next_startup() {
            run_plan(plan, &mut context);
        }

        assert_eq!(*observed.borrow(), vec![1, 3, 4]);
    }

    #[test]
    fn plan_ids_are_shared_between_all_queues() {
        let observed = Rc::new(RefCell::new(Vec::new()));
        let mut plan_queue = PlanQueue::new();
        let regular_id = plan_queue.add_plan(
            1.0,
            callback(1, Rc::clone(&observed)),
            ExecutionPhase::Normal,
            false,
        );
        let shutdown_id =
            plan_queue.add_shutdown_plan(callback(2, Rc::clone(&observed)), ExecutionPhase::Normal);
        let startup_id =
            plan_queue.add_startup_plan(callback(3, Rc::clone(&observed)), ExecutionPhase::Normal);

        assert_ne!(regular_id, shutdown_id);
        assert_ne!(shutdown_id, startup_id);
        assert_eq!(regular_id.0, 0);
        assert_eq!(shutdown_id.0, 1);
        assert_eq!(startup_id.0, 2);
    }
}
