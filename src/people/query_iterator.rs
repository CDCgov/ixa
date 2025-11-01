use std::collections::VecDeque;

use super::Query;
use crate::prelude::*;

pub trait PersonIdCb: for<'a> FnMut(&'a Context, PersonId) -> Option<()> {}
impl<F: FnMut(&Context, PersonId) -> Option<()> + 'static> PersonIdCb for F {}

pub trait PersonIdCbMut: for<'a> FnMut(&'a mut Context, PersonId) -> Option<()> {}
impl<T> PersonIdCbMut for T where T: for<'a> FnMut(&'a mut Context, PersonId) -> Option<()> {}

enum QueryState {
    All { next_index: usize },
    Filtered { remaining: VecDeque<PersonId> },
}

fn initial_state<Q: Query>(context: &Context, query: Q) -> QueryState {
    if Q::is_empty() {
        QueryState::All { next_index: 0 }
    } else {
        // TODO: This is super not optimized.
        #[allow(deprecated)]
        let query_result = context.query_people(query);
        QueryState::Filtered {
            remaining: VecDeque::from(query_result),
        }
    }
}

fn next_from_state(context: &Context, state: &mut QueryState) -> Option<PersonId> {
    match state {
        QueryState::All { next_index } => {
            if *next_index < context.get_current_population() {
                let person = PersonId(*next_index);
                *next_index += 1;
                Some(person)
            } else {
                None
            }
        }
        QueryState::Filtered { remaining } => remaining.pop_front(),
    }
}

pub struct ImmutableQueryIterator<'a, Q: Query> {
    context: &'a Context,
    state: QueryState,
    callback: Option<Box<dyn PersonIdCb + 'a>>,
    _marker: std::marker::PhantomData<Q>,
    stop_requested: bool,
}

impl<'a, Q: Query> ImmutableQueryIterator<'a, Q> {
    fn new(context: &'a Context, query: Q, callback: Option<Box<dyn PersonIdCb + 'a>>) -> Self {
        Self {
            context,
            state: initial_state(context, query),
            callback,
            _marker: std::marker::PhantomData,
            stop_requested: false,
        }
    }
}

impl<'a, Q: Query> Iterator for ImmutableQueryIterator<'a, Q> {
    type Item = PersonId;

    fn next(&mut self) -> Option<Self::Item> {
        if self.stop_requested {
            return None;
        }

        let person = next_from_state(self.context, &mut self.state);
        if let Some(callback) = &mut self.callback {
            if callback(self.context, person?).is_none() {
                self.stop_requested = true;
            }
        }
        person
    }
}

#[allow(dead_code)]
pub struct MutableQueryIterator<'a, Q: Query> {
    context: &'a mut Context,
    state: QueryState,
    callback: Option<Box<dyn PersonIdCbMut + 'a>>,
    stop_requested: bool,
    _marker: std::marker::PhantomData<Q>,
}

impl<'a, Q: Query> MutableQueryIterator<'a, Q> {
    fn new(
        context: &'a mut Context,
        query: Q,
        callback: Option<Box<dyn PersonIdCbMut + 'a>>,
    ) -> Self {
        let state = {
            let context_ref: &Context = context;
            initial_state(context_ref, query)
        };
        Self {
            context,
            state,
            callback,
            stop_requested: false,
            _marker: std::marker::PhantomData,
        }
    }
}

impl<'a, Q: Query> Iterator for MutableQueryIterator<'a, Q> {
    type Item = PersonId;

    fn next(&mut self) -> Option<Self::Item> {
        if self.stop_requested {
            return None;
        }

        let context_ref: &Context = &*self.context;
        let person = next_from_state(context_ref, &mut self.state)?;

        if let Some(callback) = &mut self.callback {
            if callback(self.context, person).is_none() {
                self.stop_requested = true;
            }
        }

        Some(person)
    }
}

pub enum QueryIterator<'a, Q: Query> {
    Immutable(ImmutableQueryIterator<'a, Q>),
    #[allow(dead_code)]
    Mutable(MutableQueryIterator<'a, Q>),
}

impl<'a, Q: Query> QueryIterator<'a, Q> {
    pub fn new(context: &'a Context, query: Q) -> Self {
        QueryIterator::Immutable(ImmutableQueryIterator::new(context, query, None))
    }

    pub fn new_with_callback(
        context: &'a Context,
        query: Q,
        callback: impl PersonIdCb + 'a,
    ) -> Self {
        QueryIterator::Immutable(ImmutableQueryIterator::new(
            context,
            query,
            Some(Box::new(callback) as Box<dyn PersonIdCb + 'a>),
        ))
    }

    pub fn new_mut_with_callback(
        context: &'a mut Context,
        query: Q,
        callback: impl PersonIdCbMut + 'a,
    ) -> Self {
        QueryIterator::Mutable(MutableQueryIterator::new(
            context,
            query,
            Some(Box::new(callback) as Box<dyn PersonIdCbMut + 'a>),
        ))
    }
}

impl<'a, Q: Query> Iterator for QueryIterator<'a, Q> {
    type Item = PersonId;

    fn next(&mut self) -> Option<Self::Item> {
        match self {
            QueryIterator::Immutable(iter) => iter.next(),
            QueryIterator::Mutable(iter) => iter.next(),
        }
    }
}
#[cfg(test)]
mod test {
    use std::cell::Cell;
    use std::rc::Rc;

    use super::*;
    use crate::{HashSet, HashSetExt};

    define_person_property_with_default!(Age, u8, 0);

    #[test]
    fn test_query_iterator() {
        let mut context = Context::new();
        let mut expected = HashSet::new();
        for _ in 0..10 {
            let p = context.add_person(()).unwrap();
            expected.insert(p);
        }

        for person_id in context.iter_query(()) {
            expected.remove(&person_id);
        }
        assert!(expected.is_empty());
    }

    #[test]
    fn test_query_iterator_filtered() {
        let mut context = Context::new();
        let q = (Age, 40);
        // Add people that don't match the query
        for _ in 0..10 {
            context.add_person((Age, 20)).unwrap();
        }
        // Add people that match the query, which we expect the iterator to return
        let mut expected = HashSet::new();
        for _ in 0..12 {
            let p = context.add_person(q).unwrap();
            expected.insert(p);
        }

        for person_id in context.iter_query((Age, 40)) {
            expected.remove(&person_id);
        }
        assert!(expected.is_empty());
    }

    #[test]
    fn test_query_iterator_with_callback_stops_after_signal() {
        let mut context = Context::new();
        for _ in 0..5 {
            context.add_person(()).unwrap();
        }

        let callback_counter = Rc::new(Cell::new(0));
        let stop_after = 3usize;

        context.with_person_mut((), |ctx, person_id| {
            let counter = Rc::clone(&callback_counter);
            counter.set(counter.get() + 1);
            ctx.add_plan(1.0, |_| {});
            if person_id.0 + 1 == stop_after {
                None
            } else {
                Some(())
            }
        });
        assert_eq!(callback_counter.get(), stop_after);
    }
}
