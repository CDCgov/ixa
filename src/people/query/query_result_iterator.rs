//! A `QueryResultIterator` encapsulates the execution of a query, presenting the results as
//! an iterator. A `QueryResultIterator` holds an immutable reference to the `Context`, so
//! operations that mutate `Context` will be forbidden by the compiler statically. The results
//! can be collected into a `Vec` (or other container) with the `collect` idiom for use cases
//! where you want a mutable copy of the result set. If you don't need a mutable copy, use
//! `Context::with_query_results` instead, as it is much more efficient for indexed queries.

use crate::people::data::PeopleIterator;
use crate::people::query::source_set::{SourceIterator, SourceSet};
use crate::{HashSet, PersonId};
use std::cell::Ref;

pub struct QueryResultIterator<'c> {
    source: SourceIterator<'c>,
    sources: Vec<SourceSet<'c>>,
}

impl<'c> QueryResultIterator<'c> {
    /// Create a new empty `QueryResultIterator` for situations where you know
    /// there are no results but need a `QueryResultIterator`.
    pub fn empty() -> QueryResultIterator<'c> {
        QueryResultIterator {
            source: SourceIterator::Empty,
            sources: vec![],
        }
    }

    /// Create a new `QueryResultIterator` that iterates over the entire population.
    /// This is used, for example, when the query is the empty query.
    pub(super) fn from_population_iterator(iter: PeopleIterator) -> Self {
        QueryResultIterator {
            source: SourceIterator::WholePopulation(iter),
            sources: vec![],
        }
    }

    /// Create a new `QueryResultIterator` from a provided list of sources.
    /// The sources need not be sorted.
    pub fn from_sources(mut sources: Vec<SourceSet<'c>>) -> Self {
        if sources.is_empty() {
            return Self::empty();
        }

        sources.sort_unstable_by_key(|x| x.len());
        let source = sources.remove(0).into_iter();
        QueryResultIterator { source, sources }
    }

    pub fn from_index_set(set: Ref<'c, HashSet<PersonId>>) -> QueryResultIterator<'c> {
        QueryResultIterator {
            source: SourceSet::IndexSet(set).into_iter(),
            sources: vec![],
        }
    }
}

impl<'a> Iterator for QueryResultIterator<'a> {
    type Item = PersonId;

    fn next(&mut self) -> Option<Self::Item> {
        // 4. Walk over the iterator and return a person iff:
        //    (1) they exist in all the indexes
        //    (2) they match the unindexed properties
        'outer: for person in self.source.by_ref() {
            // (1) check all the indexes
            for source in &self.sources {
                if !source.contains(person) {
                    continue 'outer;
                }
            }

            // This person matches.
            return Some(person);
        }

        None
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let (lower, upper) = self.source.size_hint();
        if self.sources.is_empty() {
            (lower, upper)
        } else {
            // The intersection may be empty but cannot have more than the
            // upper bound of the source set.
            (0, upper)
        }
    }
}
