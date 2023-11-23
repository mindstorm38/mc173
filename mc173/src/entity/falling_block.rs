//! Falling block entity logic implementation.

use glam::DVec3;

use crate::world::World;

use super::{FallingBlockEntity, Size};


impl FallingBlockEntity {

    /// Tick the falling block entity.
    pub fn tick_falling_block(&mut self, world: &mut World, id: u32) {

        self.tick_base(world, id, Size::new_centered(1.0, 1.0));

        if self.kind.block_id == 0 {
            world.remove_entity(id);
            return;
        }

        self.lifetime += 1;
        
        self.vel_dirty = true;
        self.vel.y -= 0.04;
        self.update_pos_move(world, self.vel, 0.0);

        if self.on_ground {
            self.vel *= DVec3::new(0.7, -0.5, 0.7);
            world.remove_entity(id);
            // TODO: Place block or drop item.
        } else if self.lifetime > 100 {
            // TODO: Drop item.
            world.remove_entity(id);
        }

    }

}
