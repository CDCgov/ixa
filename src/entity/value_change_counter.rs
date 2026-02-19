use std::any::Any;
use std::hash::Hash;
use std::marker::PhantomData;

use crate::entity::property::Property;
use crate::entity::property_list::PropertyList;
use crate::entity::{Entity, EntityId};
use crate::{Context, HashMap};

pub trait ValueChangeCounter<E: Entity, P: Property<E>>: 'static {
    fn update(&mut self, entity_id: EntityId<E>, new_property_value: P, context: &Context);
    fn as_any_mut(&mut self) -> &mut dyn Any;
}

pub struct StratifiedValueChangeCounter<E: Entity, PL: PropertyList<E>, P: Property<E>> {
    counts: HashMap<(PL, P), usize>,
    _phantom: PhantomData<E>,
}

impl<E: Entity, PL: PropertyList<E>, P: Property<E>> StratifiedValueChangeCounter<E, PL, P> {
    pub fn new() -> Self {
        Self {
            counts: HashMap::default(),
            _phantom: PhantomData,
        }
    }

    pub fn get_count(&self, stratum: PL, value: P) -> usize
    where
        PL: Eq + Hash,
        P: Eq + Hash,
    {
        *self.counts.get(&(stratum, value)).unwrap_or(&0)
    }

    pub fn iter(&self) -> impl Iterator<Item = (&(PL, P), &usize)> {
        self.counts.iter()
    }

    pub fn clear(&mut self) {
        self.counts.clear();
    }
}

impl<E: Entity, PL: PropertyList<E>, P: Property<E>> Default
    for StratifiedValueChangeCounter<E, PL, P>
{
    fn default() -> Self {
        Self::new()
    }
}

impl<E: Entity, PL: PropertyList<E>, P: Property<E>> ValueChangeCounter<E, P>
    for StratifiedValueChangeCounter<E, PL, P>
where
    PL: Eq + Hash,
    P: Eq + Hash,
{
    fn update(&mut self, entity_id: EntityId<E>, new_property_value: P, context: &Context) {
        let stratum = PL::get_values_for_entity(context, entity_id);
        let key = (stratum, new_property_value);
        let count = self.counts.entry(key).or_insert(0);
        *count += 1;
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}
