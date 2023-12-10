//! Player entity implementation.

use crate::world::World;

use super::{PlayerEntity, Size};


impl PlayerEntity {

    /// Tick the player entity.
    pub fn tick_player(&mut self, world: &mut World, id: u32) {
        
        self.tick_living(world, id, Size::new(0.6, 1.8), |_, _| {});
        
        // Player is manually moved from external logic, we still need to update the 
        // bounding box to account for the new position.
        self.update_bounding_box_from_pos();

    }

}
