//! Pig entity.

use crate::world::World;

use super::{PigEntity, Size};


impl PigEntity {
    
    /// Tick the pig entity.
    pub fn tick_pig(&mut self, world: &mut World, id: u32) {
        
        self.tick_living(world, id, Size::new(0.9, 0.9), Self::update_animal_ai);
        self.update_living_pos(world, 0.5);

    }

}
