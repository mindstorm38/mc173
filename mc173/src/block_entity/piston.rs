//! Moving piston block entity.

use glam::IVec3;

use crate::block;
use crate::world::World;
use crate::geom::Face;


#[derive(Debug, Clone)]
pub struct PistonBlockEntity {
    /// The block id of the moving piston block.
    pub block: u8,
    /// The block metadata of the moving piston block.
    pub metadata: u8,
    /// Face toward the block is moving.
    pub face: Face,
    /// Progress of the move animation.
    pub progress: f32,
    /// True when the piston is extending, false when retracting.
    pub extending: bool,
}

impl Default for PistonBlockEntity {
    fn default() -> Self {
        Self { 
            block: 0, 
            metadata: 0, 
            face: Face::PosY,
            progress: 0.0,
            extending: false,
        }
    }
}

impl PistonBlockEntity {

    pub fn tick(&mut self, world: &mut World, pos: IVec3) {

        if self.progress >= 1.0 {
            // TODO: Handle entity push
            world.remove_block_entity(pos);
            if world.is_block(pos, block::PISTON_MOVING) {
                world.set_block_notify(pos, self.block, self.metadata);
            }
        } else {
            self.progress += 0.5;
            if self.extending {
                // TODO: Handle entity push
            }
        }

    }

}
