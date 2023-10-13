//! Fluid block special functions (mostly for water).

use glam::{IVec3, DVec3};

use crate::world::World;


/// Calculate the fluid height based on its metadata.
#[inline]
pub fn calc_fluid_height(metadata: u8) -> f32 {
    if metadata >= 8 {
        0.0
    } else {
        (metadata + 1) as f32 / 9.0
    }
}


/// Calculate the velocity applied by a fluid block at the given position. You must 
/// ensure before calling that this position contains a fluid block.
pub fn calc_fluid_velocity(world: &mut World, pos: IVec3) -> Option<DVec3> {

    // TODO:
    
    let mut ret = DVec3::ZERO;
    let (center_block, center_metadata) = world.block_and_metadata(pos)?;

    // Fetch all block around the block...
    let mut delta = IVec3::new(-1, 0, 0);
    for _ in 0..3 {
        
        let (block, metadata) = world.block_and_metadata(pos + delta)?;
        if block == center_block {

        }

        delta = IVec3::new(-delta.z, 0, delta.x);

    }

    Some(ret.normalize())

}
