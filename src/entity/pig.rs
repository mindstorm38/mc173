//! Pig entity.

use crate::world::World;

use super::{Base, Living, Creature, EntityLogic, Size};


#[derive(Debug, Default)]
pub struct Pig {
    /// True when the pig has a saddle.
    pub saddle: bool,
}

/// A player entity.
pub type PigEntity = Base<Living<Creature<Pig>>>;

impl EntityLogic for PigEntity {

    fn tick(&mut self, world: &mut World) {
        
        // Entity.onEntityUpdate()
        self.update_entity(world, Size::new(0.9, 0.9));
        // EntityLiving.onLivingUpdate()
        self.update_living_entity(world, Self::update_creature_ai);
        // EntityLiving.moveEntityWithHeading()
        self.move_living_entity(world, 0.5);

    }

}
