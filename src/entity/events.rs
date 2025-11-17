/*!

`EntityCreatedEvent` and `EntityPropertyChangeEvent` types in analogy to `PersonCreatedEvent` and `PersonPropertyChangeEvent`.

*/

use ixa_derive::IxaEvent;

use crate::entity::property::Property;
use crate::entity::{Entity, EntityId};
use crate::IxaEvent;

/// Emitted when a new entity is created.
/// These should not be emitted outside this module.
#[derive(Clone, Copy, IxaEvent)]
#[allow(clippy::manual_non_exhaustive)]
pub struct EntityCreatedEvent<E: Entity> {
    /// The [`EntityId<E>`] of the new entity.
    pub entity_id: EntityId<E>,
}

/// Emitted when a property is updated.
/// These should not be emitted outside this module.
#[derive(Copy, Clone)]
#[allow(clippy::manual_non_exhaustive)]
pub struct PropertyChangeEvent<E: Entity, P: Property<E>> {
    /// The [`EntityId<E>`] that changed
    pub entity_id: EntityId<E>,
    /// The new value
    pub current: P,
    /// The old value
    pub previous: P,
}

// impl<E: Entity, P: Property<E>> IxaEvent for PropertyChangeEvent<E, P> {
//     fn on_subscribe(context: &mut Context) {
//         if P::is_derived() {
//             context.register_property::<T>();
//         }
//     }
// }
