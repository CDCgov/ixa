use crate::{context::Context, define_data_plugin, error::IxaError, people::PersonId};
use std::{any::{Any, TypeId}, collections::HashMap};

#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Edge<T: Sized> {
    neighbor: PersonId,
    weight: f32,
    inner: T,
}

pub trait EdgeType {
    type Value: Sized + Default + Copy;
}

#[derive(Default)]
struct PersonNetwork {
    // A vector of vectors of NetworkEdge, indexed by edge type.
    neighbors: HashMap<TypeId, Box<dyn Any>>,
}

struct NetworkData {
    network: Vec<PersonNetwork>,
}

impl NetworkData {
    fn new() -> Self {
        NetworkData {
            network: Vec::new(),
        }
    }

    fn add_edge<T: EdgeType + 'static>(
        &mut self,
        person: PersonId,
        neighbor: PersonId,
        weight: f32,
        inner: T::Value
    ) -> Result<(), IxaError> {
        if person == neighbor {
            return Err(IxaError::IxaError(String::from("Cannot make edge to self")));
        }

        if weight.is_infinite() || weight.is_nan() || weight.is_sign_negative() {
            return Err(IxaError::IxaError(String::from("Invalid weight")));
        }

        // Make sure we have data for this person.
        if person.id >= self.network.len() {
            self.network.resize_with(person.id + 1, Default::default);
        }

        let entry = self.network[person.id]
            .neighbors
            .entry(TypeId::of::<T>())
            .or_insert_with(|| Box::new(Vec::<Edge<T::Value>>::new()));
        let edges : &mut Vec<Edge<T::Value>> = entry.downcast_mut().expect("Type mismatch");

        for edge in edges.iter_mut() {
            if edge.neighbor == neighbor {
                return Err(IxaError::IxaError(String::from("Edge already exists")));
            }
        }

        edges.push(Edge{ neighbor,
                         weight,
                         inner });
        Ok(())
    }

    fn remove_edge<T: EdgeType + 'static>(
        &mut self,
        person: PersonId,
        neighbor: PersonId,
    ) -> Result<(), IxaError> {

        if person.id >= self.network.len() {
            return Err(IxaError::IxaError(String::from("Edge does not exist")));
        }

        let entry = match self.network[person.id]
            .neighbors.get_mut(&TypeId::of::<T>()) {
                None => {
                    return Err(IxaError::IxaError(String::from("Edge does not exist")));
                },
                Some(entry) => entry
            };

        let edges : &mut Vec<Edge<T::Value>> = entry.downcast_mut().expect("Type mismatch");
        for index in 0..edges.len() {
            if edges[index].neighbor == neighbor {
                edges.remove(index);
                return Ok(());
            }
        }

        Err(IxaError::IxaError(String::from("Edge does not exist")))
    }


    fn get_edge<T: EdgeType + 'static>(&self, person: PersonId, neighbor: PersonId) -> Option<&Edge<T::Value>> {
        if person.id >= self.network.len() {
            return None;
        }

        let entry = self.network[person.id].neighbors.get(&TypeId::of::<T>())?;
        let edges : &Vec<Edge<T::Value>> = entry.downcast_ref().expect("Type mismatch");        
        for edge in edges {
            if edge.neighbor == neighbor {
                return Some(edge);
            }
        }

        None
    }

    fn get_edges<T: EdgeType + 'static>(&self, person: PersonId) -> Vec<Edge<T::Value>> {
        if person.id >= self.network.len() {
            return Vec::new();
        }

        let entry = self.network[person.id].neighbors.get(&TypeId::of::<T>());
        if entry.is_none() {
            return Vec::new();
        }

        let edges : &Vec<Edge<T::Value>> = entry.unwrap().downcast_ref().expect("Type mismatch");        
        edges.clone()
    }
}

macro_rules! define_edge_type {
    ($edge_type:ident, $value:ty) => {
        #[derive(Debug, Copy, Clone)]
        pub struct $edge_type;

        impl $crate::network::EdgeType for $edge_type {
            type Value = $value;
        }
    }
}
    
define_data_plugin!(NetworkPlugin, NetworkData, NetworkData::new());

pub trait ContextNetworkExt {
    fn add_edge<T: EdgeType + 'static>(
        &mut self,
        person: PersonId,
        neighbor: PersonId,
        weight: f32,
        inner: T::Value,
    ) -> Result<(), IxaError>;
    fn add_edge_bidi<T: EdgeType + 'static>(
        &mut self,
        person1: PersonId,
        person2: PersonId,
        weight: f32,
        inner: T::Value,
    ) -> Result<(), IxaError>;
    fn remove_edge<T: EdgeType + 'static>(
        &mut self,
        person: PersonId,
        neighbor: PersonId,
    ) -> Result<(), IxaError>;
    fn get_edges<T: EdgeType + 'static>(&self, person: PersonId) -> Vec<Edge<T::Value>>;
    fn get_matching_edges<T: EdgeType + 'static>(&self, person: PersonId,
                                                 filter: impl Fn(&Context, &Edge<T::Value>)
                                                            -> bool + 'static)
                                                 -> Vec<Edge<T::Value>>;
    fn get_edge<T: EdgeType + 'static>(&self, person: PersonId, neighbor: PersonId) -> Option<&Edge<T::Value>>;
}

impl ContextNetworkExt for Context {
    fn add_edge<T: EdgeType + 'static>(
        &mut self,
        person: PersonId,
        neighbor: PersonId,
        weight: f32,
        inner: T::Value,
    ) -> Result<(), IxaError> {
        let data_container = self.get_data_container_mut(NetworkPlugin);
        data_container.add_edge::<T>(person, neighbor, weight, inner)
    }
    
    fn add_edge_bidi<T: EdgeType + 'static>(
        &mut self,
        person1: PersonId,
        person2: PersonId,
        weight: f32,
        inner: T::Value,
    ) -> Result<(), IxaError> {
        let data_container = self.get_data_container_mut(NetworkPlugin);
        data_container.add_edge::<T>(person1, person2, weight, inner)?;
        data_container.add_edge::<T>(person2, person1, weight, inner)
    }

    fn remove_edge<T: EdgeType + 'static>(
        &mut self,
        person: PersonId,
        neighbor: PersonId,
    ) -> Result<(), IxaError> {
        let data_container = self.get_data_container(NetworkPlugin);
        // Generate an error if not initialized.
        if data_container.is_none() {
            return Err(IxaError::IxaError(String::from("Network not initialized")));
        }
        let data_container = self.get_data_container_mut(NetworkPlugin);
        data_container.remove_edge::<T>(person, neighbor)
    }

    fn get_edge<T: EdgeType + 'static>(&self, person: PersonId, neighbor: PersonId) -> Option<&Edge<T::Value>> {
        let data_container = self.get_data_container(NetworkPlugin);

        match data_container {
            None => None,
            Some(data_container) => data_container.get_edge::<T>(person, neighbor),
        }
    }

    fn get_edges<T: EdgeType + 'static>(&self, person: PersonId) -> Vec<Edge<T::Value>> {
        let data_container = self.get_data_container(NetworkPlugin);

        match data_container {
            None => Vec::new(),
            Some(data_container) => data_container.get_edges::<T>(person),
        }
    }

    fn get_matching_edges<T: EdgeType + 'static>(&self, person: PersonId,
                                                 filter: impl Fn(&Context, &Edge<T::Value>)
                                                            -> bool + 'static)
                                                 -> Vec<Edge<T::Value>> {
        let edges = self.get_edges::<T>(person);
        let mut result = Vec::new();
        for edge in &edges {
            if filter(self, edge) {
                result.push(*edge);
            }
        }
        result
    }
}

#[cfg(test)]
#[allow(clippy::float_cmp)]
// Tests for the inner core.
mod test_inner {
    use super::{Edge, NetworkData};
    use crate::error::IxaError;
    use crate::people::PersonId;

    define_edge_type!(EdgeType1, ());
    define_edge_type!(EdgeType2, ());
    define_edge_type!(EdgeType3, bool);

    #[test]
    fn add_edge() {
        let mut nd = NetworkData::new();

        nd.add_edge::<EdgeType1>(PersonId { id: 1 }, PersonId { id: 2 }, 0.01, ())
            .unwrap();
        let edge = nd
            .get_edge::<EdgeType1>(PersonId { id: 1 }, PersonId { id: 2 })
            .unwrap();
        assert_eq!(edge.weight, 0.01);
    }

    #[test]
    fn add_edge_with_inner() {
        let mut nd = NetworkData::new();

        nd.add_edge::<EdgeType3>(PersonId { id: 1 }, PersonId { id: 2 }, 0.01, true)
            .unwrap();
        let edge = nd
            .get_edge::<EdgeType3>(PersonId { id: 1 }, PersonId { id: 2 })
            .unwrap();
        assert_eq!(edge.weight, 0.01);
        assert!(edge.inner);
    }

    #[test]
    fn add_two_edges() {
        let mut nd = NetworkData::new();

        nd.add_edge::<EdgeType1>(PersonId { id: 1 }, PersonId { id: 2 }, 0.01, ())
            .unwrap();
        nd.add_edge::<EdgeType1>(PersonId { id: 1 }, PersonId { id: 3 }, 0.02, ())
            .unwrap();
        let edge = nd
            .get_edge::<EdgeType1>(PersonId { id: 1 }, PersonId { id: 2 })
            .unwrap();
        assert_eq!(edge.weight, 0.01);
        let edge = nd
            .get_edge::<EdgeType1>(PersonId { id: 1 }, PersonId { id: 3 })
            .unwrap();
        assert_eq!(edge.weight, 0.02);

        let edges = nd.get_edges::<EdgeType1>(PersonId { id: 1 });
        assert_eq!(
            edges,
            vec![
                Edge {
                    neighbor: PersonId { id: 2 },
                    weight: 0.01,
                    inner: ()
                },
                Edge{
                    neighbor: PersonId { id: 3 },
                    weight: 0.02,
                    inner: ()
                }
            ]
        );
    }

    #[test]
    fn add_two_edge_types() {
        let mut nd = NetworkData::new();

        nd.add_edge::<EdgeType1>(PersonId { id: 1 }, PersonId { id: 2 }, 0.01, ())
            .unwrap();
        nd.add_edge::<EdgeType2>(PersonId { id: 1 }, PersonId { id: 2 }, 0.02, ())
            .unwrap();
        let edge = nd
            .get_edge::<EdgeType1>(PersonId { id: 1 }, PersonId { id: 2 })
            .unwrap();
        assert_eq!(edge.weight, 0.01);
        let edge = nd
            .get_edge::<EdgeType2>(PersonId { id: 1 }, PersonId { id: 2 })
            .unwrap();
        assert_eq!(edge.weight, 0.02);

        let edges = nd.get_edges::<EdgeType1>(PersonId { id: 1 });
        assert_eq!(
            edges,
            vec![Edge {
                neighbor: PersonId { id: 2 },
                weight: 0.01,
                inner: ()
            }]
        );
    }

    #[test]
    fn add_edge_twice_fails() {
        let mut nd = NetworkData::new();

        nd.add_edge::<EdgeType1>(PersonId { id: 1 }, PersonId { id: 2 }, 0.01, ())
            .unwrap();
        let edge = nd
            .get_edge::<EdgeType1>(PersonId { id: 1 }, PersonId { id: 2 })
            .unwrap();
        assert_eq!(edge.weight, 0.01);

        assert!(matches!(nd.add_edge::<EdgeType1>(PersonId { id: 1 }, PersonId { id: 2 }, 0.02, ()),
                         Err(IxaError::IxaError(_))));
    }

    #[test]
    fn add_remove_add_edge() {
        let mut nd = NetworkData::new();

        nd.add_edge::<EdgeType1>(PersonId { id: 1 }, PersonId { id: 2 }, 0.01, ())
            .unwrap();
        let edge = nd
            .get_edge::<EdgeType1>(PersonId { id: 1 }, PersonId { id: 2 })
            .unwrap();
        assert_eq!(edge.weight, 0.01);
        
        nd.remove_edge::<EdgeType1>(PersonId { id: 1 }, PersonId { id: 2 }).unwrap();
        let edge = nd
            .get_edge::<EdgeType1>(PersonId { id: 1 }, PersonId { id: 2 });
        assert!(edge.is_none());

        nd.add_edge::<EdgeType1>(PersonId { id: 1 }, PersonId { id: 2 }, 0.02, ())
            .unwrap();
        let edge = nd
            .get_edge::<EdgeType1>(PersonId { id: 1 }, PersonId { id: 2 })
            .unwrap();
        assert_eq!(edge.weight, 0.02);
    }

    #[test]
    fn remove_nonexistent_edge() {
        let mut nd = NetworkData::new();
        assert!(matches!(nd.remove_edge::<EdgeType1>(PersonId { id: 1 }, PersonId { id: 2 }),
                                                     Err(IxaError::IxaError(_))));
    }
    
    #[test]
    fn add_edge_to_self() {
        let mut nd = NetworkData::new();

        let result = nd.add_edge::<EdgeType1>(PersonId { id: 1 }, PersonId { id: 1 }, 0.01, ());
        assert!(matches!(result, Err(IxaError::IxaError(_))));
    }

    #[test]
    fn add_edge_bogus_weight() {
        let mut nd = NetworkData::new();

        let result = nd.add_edge::<EdgeType1>(PersonId { id: 1 }, PersonId { id: 2 }, -1.0, ());
        assert!(matches!(result, Err(IxaError::IxaError(_))));

        let result = nd.add_edge::<EdgeType1>(PersonId { id: 1 }, PersonId { id: 2 }, f32::NAN, ());
        assert!(matches!(result, Err(IxaError::IxaError(_))));

        let result =
            nd.add_edge::<EdgeType1>(PersonId { id: 1 }, PersonId { id: 2 }, f32::INFINITY, ());
        assert!(matches!(result, Err(IxaError::IxaError(_))));
    }
}


#[cfg(test)]
#[allow(clippy::float_cmp)]
// Tests for the API.
mod test_api {
    use crate::context::Context;
    use crate::network::{ContextNetworkExt, Edge};
    use crate::people::{ContextPeopleExt, PersonId, define_person_property};
    use crate::error::IxaError;
    
    define_edge_type!(EdgeType1, ());
    define_person_property!(Age, u8);
    
    fn setup() -> (Context, PersonId, PersonId) {
        let mut context = Context::new();
        let person1 = context.add_person((Age, 1)).unwrap();
        let person2 = context.add_person((Age, 2)).unwrap();

        (context, person1, person2)
    }

    #[test]
    fn add_edge() {
        let (mut context, person1, person2) = setup();

        context
            .add_edge::<EdgeType1>(person1, person2, 0.01, ())
            .unwrap();
        assert_eq!(context.get_edge::<EdgeType1>(person1, person2).unwrap().weight, 0.01);
        assert_eq!(
            context.get_edges::<EdgeType1>(person1),
            vec![Edge {
                neighbor: person2,
                weight: 0.01,
                inner: ()
            }]
        );
    }

    #[test]
    fn remove_edge() {
        let (mut context, person1, person2) = setup();
        // Check that we get an error if nothing has been added.
        
        assert!(matches!(context.remove_edge::<EdgeType1>(person1, person2),
                Err(IxaError::IxaError(_))));
        context
            .add_edge::<EdgeType1>(person1, person2, 0.01, ())
            .unwrap();
        context.remove_edge::<EdgeType1>(person1, person2).unwrap();
        assert!(context.get_edge::<EdgeType1>(person1, person2).is_none());
        assert_eq!(context.get_edges::<EdgeType1>(person1).len(), 0);
    }
    
    #[test]
    fn add_edge_bidi() {
        let (mut context, person1, person2) = setup();

        context
            .add_edge_bidi::<EdgeType1>(person1, person2, 0.01, ())
            .unwrap();
        assert_eq!(context.get_edge::<EdgeType1>(person1, person2).unwrap().weight, 0.01);
        assert_eq!(context.get_edge::<EdgeType1>(person2, person1).unwrap().weight, 0.01);

    }

    #[test]
    fn add_edge_different_weights() {
        let (mut context, person1, person2) = setup();

        context
            .add_edge::<EdgeType1>(person1, person2, 0.01, ())
            .unwrap();
        context
            .add_edge::<EdgeType1>(person2, person1, 0.02, ())
            .unwrap();
        assert_eq!(context.get_edge::<EdgeType1>(person1, person2).unwrap().weight, 0.01);
        assert_eq!(context.get_edge::<EdgeType1>(person2, person1).unwrap().weight, 0.02);
    }

    #[test]
    fn get_matching_edges_weight() {
        let (mut context, person1, person2) = setup();
        let person3 = context.add_person((Age, 3)).unwrap();
        
        context
            .add_edge::<EdgeType1>(person1, person2, 0.01, ())
            .unwrap();
        context
            .add_edge::<EdgeType1>(person1, person3, 0.03, ())
            .unwrap();
        let edges = context.get_matching_edges::<EdgeType1>(person1, | _context, edge| {
            edge.weight > 0.01
        });
        assert_eq!(edges.len(), 1);
        assert_eq!(edges[0].neighbor, person3);
    }

    #[test]
    fn get_matching_edges_property() {
        let (mut context, person1, person2) = setup();
        let person3 = context.add_person((Age, 3)).unwrap();
        
        context
            .add_edge::<EdgeType1>(person1, person2, 0.01, ())
            .unwrap();
        context
            .add_edge::<EdgeType1>(person1, person3, 0.03, ())
            .unwrap();
        let edges = context.get_matching_edges::<EdgeType1>(person1, | context, edge| {
            context.match_person(edge.neighbor, (Age, 3))
        });
        assert_eq!(edges.len(), 1);
        assert_eq!(edges[0].neighbor, person3);
    }
    
}
