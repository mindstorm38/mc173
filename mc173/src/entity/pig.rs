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

    fn size(&mut self) -> Size {
        Size::new(0.9, 0.9)
    }
    
    fn tick(&mut self, world: &mut World) {
        
        // Entity.onEntityUpdate()
        self.update(world);
        // EntityLiving.onLivingUpdate()
        self.update_living(world, Self::update_animal_ai);
        // EntityLiving.moveEntityWithHeading()
        self.update_living_position(world, 0.5);

    }

}
