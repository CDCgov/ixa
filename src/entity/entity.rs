use crate::define_type_store;
use crate::prelude::*;
use crate::type_store::TypeIndex;
use crate::vec_cell::VecCell;

define_type_store!(EntityStore<EntityMarker, EntityData>);
define_type_store!(EntityPropertyStore<EntityProperty>);

define_data_plugin!(EntityPlugin, EntityStore, EntityStore::new());
define_data_plugin!(
    EntityPropertyPlugin,
    EntityPropertyStore,
    EntityPropertyStore::new()
);

pub trait Entity: 'static + TypeIndex<Category = EntityMarker, Data = EntityData> {}

pub trait Property: 'static + Copy {
    type Value: Copy;
}

pub trait PropertyFor<E: Entity>: Property {
    fn initializer(context: &Context) -> Self::Value;
    fn type_index() -> usize;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct EntityId<T: Entity> {
    id: usize,
    _marker: std::marker::PhantomData<T>,
}

#[derive(Default)]
pub struct EntityData {
    count: usize,
}

impl EntityData {
    pub fn increment<E: Entity>(&mut self) -> EntityId<E> {
        self.count += 1;
        EntityId::<E>::new(self.count - 1)
    }
    pub fn get_count(&self) -> usize {
        self.count
    }
}

impl<T: Entity> EntityId<T> {
    pub fn new(id: usize) -> Self {
        Self {
            id,
            _marker: std::marker::PhantomData,
        }
    }
    pub fn id(&self) -> usize {
        self.id
    }
}

pub trait EntityContextExt: PluginContext {
    fn get_entity_data<E: Entity>(&self) -> &EntityData {
        self.get_data(EntityPlugin)
            .get_or_init::<E, _>(EntityData::default)
    }
    fn get_entity_data_mut<E: Entity>(&mut self) -> &mut EntityData {
        self.get_data_mut(EntityPlugin)
            .get_or_init_mut::<E, _>(EntityData::default)
    }
    fn entity_count<E: Entity>(&self) -> usize {
        let data = self.get_entity_data::<E>();
        data.get_count()
    }
    fn entity_iter<E: Entity>(&self) -> impl Iterator<Item = EntityId<E>> {
        let data = self.get_entity_data::<E>();
        (0..data.get_count()).map(EntityId::<E>::new)
    }
    fn add_entity<E: Entity>(&mut self) -> EntityId<E> {
        let data = self.get_entity_data_mut::<E>();
        data.increment()
    }
    fn get_property<E, P>(&self, entity_id: EntityId<E>) -> P::Value
    where
        E: Entity,
        P: PropertyFor<E> + Copy;
    fn set_property<E, P>(&mut self, entity_id: EntityId<E>, value: P::Value)
    where
        E: Entity,
        P: PropertyFor<E> + Copy;
}

impl EntityContextExt for Context {
    fn get_property<E, P>(&self, entity_id: EntityId<E>) -> P::Value
    where
        E: Entity,
        P: PropertyFor<E> + Copy,
    {
        let vec = self
            .get_data(EntityPropertyPlugin)
            .get_by_index::<VecCell<Option<P::Value>>, _>(P::type_index(), VecCell::default)
            .expect("Failed to retrieve VecCell");
        let current = vec.get_or_extend(entity_id.id(), || None);
        if let Some(value) = current {
            value
        } else {
            let initialized = P::initializer(self);
            vec.set(entity_id.id(), Some(initialized));
            initialized
        }
    }
    fn set_property<E, P>(&mut self, entity_id: EntityId<E>, value: P::Value)
    where
        E: Entity,
        P: PropertyFor<E> + Copy,
    {
        self.get_data(EntityPropertyPlugin)
            .get_by_index::<VecCell<Option<P::Value>>, _>(P::type_index(), VecCell::default);
        let vec = self
            .get_data_mut(EntityPropertyPlugin)
            .get_mut_by_index::<VecCell<Option<P::Value>>>(P::type_index())
            .unwrap();
        vec.set_or_extend(entity_id.id(), Some(value), || None)
    }
}

#[cfg(test)]
mod tests {
    use super::super::Person;
    use super::*;
    use crate::{define_entity, define_entity_property, entity_property_for};

    define_entity!(pub struct Setting);
    define_entity_property!(pub struct Age: usize);
    define_entity_property!(
        pub enum InfectionStatus {
            Susceptible,
            Infected,
        }
    );

    entity_property_for!(Person => Age, default = 0);
    entity_property_for!(Person => InfectionStatus, default = InfectionStatus::Susceptible);
    entity_property_for!(Setting => Age, default = 0);

    #[test]
    fn test_add_entity() {
        let mut context = Context::new();
        assert_eq!(context.add_entity::<Person>().id(), 0);
        assert_eq!(context.add_entity::<Person>().id(), 1);

        assert_eq!(context.add_entity::<Setting>().id(), 0);
        assert_eq!(context.add_entity::<Setting>().id(), 1);
    }

    #[test]
    fn test_properties() {
        let mut context = Context::new();
        let person = context.add_entity::<Person>();
        context.set_property::<_, Age>(person, 20);
        assert_eq!(context.get_property::<_, Age>(person), 20);
        context.get_property::<_, InfectionStatus>(person);
        assert_eq!(
            context.get_property::<_, InfectionStatus>(person),
            InfectionStatus::Susceptible
        );
        context.set_property::<_, InfectionStatus>(person, InfectionStatus::Infected);
        assert_eq!(
            context.get_property::<_, InfectionStatus>(person),
            InfectionStatus::Infected
        );

        let setting = context.add_entity::<Setting>();
        context.set_property::<_, Age>(setting, 12);
        assert_eq!(context.get_property::<_, Age>(setting), 12);
    }

    #[test]
    fn test_add_property_initialize() {
        let mut context = Context::new();
        let p1 = context.add_entity::<Person>();
        let p2 = context.add_entity::<Person>();
        assert_eq!(context.get_property::<_, Age>(p2), 0);
        assert_eq!(context.get_property::<_, Age>(p1), 0);
    }
}
