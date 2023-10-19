//! Pig entity.

use crate::world::World;

use super::{PigEntity, Size};


impl PigEntity {
    
    /// Tick the pig entity.
    pub fn tick_pig(&mut self, world: &mut World) {
        
        // Entity.onEntityUpdate()
        self.tick_base(world, Size::new(0.9, 0.9));
        // // EntityLiving.onLivingUpdate()
        // self.update_living(world, Self::update_animal_ai);
        // // EntityLiving.moveEntityWithHeading()
        // self.update_living_position(world, 0.5);

    }

}
