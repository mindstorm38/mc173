//! Falling block entity logic implementation.

use glam::DVec3;

use crate::item::ItemStack;
use crate::util::Face;
use crate::world::World;

use super::{FallingBlockEntity, Size};


impl FallingBlockEntity {

    /// Tick the falling block entity.
    pub fn tick_falling_block(&mut self, world: &mut World, id: u32) {

        self.tick_base(world, id, Size::new_centered(0.98, 0.98));

        if self.kind.block_id == 0 {
            world.remove_entity(id);
            return;
        }

        self.vel_dirty = true;
        self.vel.y -= 0.04;
        self.update_pos_move(world, self.vel, 0.0);

        if self.on_ground {

            self.vel *= DVec3::new(0.7, -0.5, 0.7);
            world.remove_entity(id);

            let block_pos = self.pos.floor().as_ivec3();
            if world.can_place_block(block_pos, Face::PosY, self.kind.block_id) {
                world.set_block_notify(block_pos, self.kind.block_id, 0);
            } else {
                self.drop_stack(world, DVec3::Y, ItemStack::new_block(self.kind.block_id, 0));
            }

        } else if self.lifetime > 100 {
            world.remove_entity(id);
            self.drop_stack(world, DVec3::Y, ItemStack::new_block(self.kind.block_id, 0));
        }

    }

}
