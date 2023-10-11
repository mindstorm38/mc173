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

    pub fn new(id: u32, pos: DVec3, block_id: u8) -> Self {
        Self {
            base: BaseEntity::new(id, pos),
            fall_ticks: 0,
            block_id,
        }
    }

}

impl Entity for FallingBlockEntity {

    fn tick(&mut self, world: &mut World) {
        
        self.fall_ticks += 1;

        super::tick_gravity(&mut self.base, world);

    }

}
