//! Player entity implementation.

use crate::world::World;

use super::{PlayerEntity, Size};


impl PlayerEntity {

    /// Tick the player entity.
    pub fn tick_player(&mut self, world: &mut World) {
        
        self.tick_base(world, Size::new(0.9, 0.9));

    }

}
