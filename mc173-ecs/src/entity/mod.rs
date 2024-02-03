use bevy::ecs::component::Component;
use bevy::math::DVec3;

use crate::geom::BoundingBox;


/// Base common structure to all entities.
pub struct Base {
    /// Tell if this entity is persistent or not. A persistent entity is saved with its
    /// chunk, but non-persistent entities are no saved. For example, all player entities
    /// are typically non-persistent because these are not real entities. Some entities
    /// cannot be persistent as they are not supported by the Notchian serialization.
    pub persistent: bool,
    /// Lifetime of the entity since it was spawned in the world, it increase at every
    /// world tick.
    pub lifetime: u32,
}

pub struct Real {
    pub bb: BoundingBox,
    pub pos: DVec3,
}
