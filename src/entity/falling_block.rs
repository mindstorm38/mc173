//! Falling block entity implementation.

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
        
        self.base.fall_ticks += 1;
        self.apply_gravity(world, Size::new(1.0, 1.0));

        if self.on_ground {
            // TODO: Place block and destroy falling block.
            let _ = self.base.block_id;
        }

    }

}
