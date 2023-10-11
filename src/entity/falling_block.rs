//! Falling block entity implementation.

use glam::DVec3;

use crate::world::World;

use super::{BaseEntity, Entity};


/// An item entity.
#[derive(Debug)]
pub struct FallingBlockEntity {
    /// Base entity data.
    base: BaseEntity,
    /// Number of ticks since this block is falling.
    fall_ticks: u32,
    /// The falling block id.
    block_id: u8,
}

impl FallingBlockEntity {

    pub fn new(pos: DVec3, block_id: u8) -> Self {
        Self {
            base: BaseEntity::new(pos, 1.0, 1.0),
            fall_ticks: 0,
            block_id,
        }
    }

}

impl Entity for FallingBlockEntity {

    fn init(&mut self, id: u32) {
        self.base.id = id;
    }
    
    fn tick(&mut self, world: &mut World) {
        
        self.fall_ticks += 1;
        self.base.apply_gravity(world);

        if self.base.on_ground {
            // TODO: Place block and destroy falling block.
            let _ = self.block_id;
        }

    }

    fn base(&self) -> &BaseEntity {
        &self.base
    }

}
