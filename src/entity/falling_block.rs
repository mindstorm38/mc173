//! Falling block entity implementation.

use glam::DVec3;

use crate::entity::Size;
use crate::world::World;

use super::{EntityLogic, Base};


#[derive(Debug, Default)]
pub struct FallingBlock {
    /// Number of ticks since this block is falling.
    pub fall_ticks: u32,
    /// The falling block id.
    pub block_id: u8,
}

/// A falling block entity.
pub type FallingBlockEntity = Base<FallingBlock>;

impl EntityLogic for FallingBlockEntity {

    fn tick(&mut self, world: &mut World) {

        if self.base.block_id == 0 {
            world.kill_entity(self.id);
            return;
        }

        self.lifetime += 1;
        self.update_bounding_box(Size::new(1.0, 1.0));
        
        self.vel.y -= 0.04;
        self.move_entity(world, self.vel, 0.0);

        if self.on_ground {
            self.vel *= DVec3::new(0.7, -0.5, 0.7);
            world.kill_entity(self.id);
            // TODO: Place block or drop item.
        } else if self.lifetime > 100 {
            // TODO: Drop item.
            world.kill_entity(self.id);
        }

    }

}
