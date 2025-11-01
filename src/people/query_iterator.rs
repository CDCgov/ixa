use super::Query;
use crate::prelude::*;

enum QueryState {
    All { prev_id: Option<PersonId> },
    Filtered { query_result: Vec<PersonId> },
}

pub struct QueryIterator<'a, Q: Query> {
    _query: Q,
    context: &'a Context,
    state: QueryState,
}

impl<'a, Q: Query> QueryIterator<'a, Q> {
    pub fn new(context: &'a Context, query: Q) -> Self {
        let state = if Q::is_empty() {
            QueryState::All { prev_id: None }
        } else {
            // TODO: This is super not optimized.
            #[allow(deprecated)]
            let query_result = context.query_people(query);
            QueryState::Filtered { query_result }
        };
        QueryIterator {
            context,
            _query: query,
            state,
        }
    }
}

impl<'a, Q: Query> Iterator for QueryIterator<'a, Q> {
    type Item = PersonId;

    fn next(&mut self) -> Option<Self::Item> {
        match &mut self.state {
            QueryState::All { prev_id } => {
                let next_index = prev_id.map_or(0, |id| id.0 + 1);
                if next_index < self.context.get_current_population() {
                    *prev_id = Some(PersonId(next_index));
                    *prev_id
                } else {
                    None
                }
            }
            QueryState::Filtered { query_result } => {
                if query_result.is_empty() {
                    None
                } else {
                    Some(query_result.remove(0))
                }
            }
        }
    }
}
#[cfg(test)]
mod test {
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
}
