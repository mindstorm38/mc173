//! This module provides various functions for getting the material properties of blocks.

use crate::block;


/// Return true if a block is a full cube.
pub fn is_cube(id: u8) -> bool {
    match id {
        block::AIR |
        block::BED |
        block::PORTAL |
        block::BUTTON |
        block::CACTUS |
        block::CAKE |
        block::WOOD_DOOR |
        block::IRON_DOOR |
        block::FARMLAND |
        block::FENCE |
        block::FIRE |
        block::WHEAT |
        block::DEAD_BUSH |
        block::RED_MUSHROOM |
        block::BROWN_MUSHROOM |
        block::SAPLING |
        block::TALL_GRASS |
        block::WATER_MOVING |
        block::WATER_STILL |
        block::LAVA_MOVING |
        block::LAVA_STILL |
        block::LADDER |
        block::LEVER |
        block::PISTON |
        block::PISTON_EXT |
        block::PISTON_MOVING |
        block::WOOD_PRESSURE_PLATE |
        block::STONE_PRESSURE_PLATE |
        block::RAIL |
        block::POWERED_RAIL |
        block::DETECTOR_RAIL |
        block::REPEATER |
        block::REPEATER_LIT |
        block::REDSTONE |
        block::SUGAR_CANES |
        block::SIGN |
        block::WALL_SIGN |
        block::SNOW |
        block::WOOD_STAIR |
        block::COBBLESTONE_STAIR |
        block::SLAB |
        block::TORCH |
        block::REDSTONE_TORCH |
        block::REDSTONE_TORCH_LIT |
        block::TRAPDOOR |
        block::COBWEB => false,
        _ => true,
    }
}

/// Return true if a block is a full opaque cube.
pub fn is_opaque_cube(id: u8) -> bool {
    if is_cube(id) {
        match id {
            block::LEAVES |
            block::GLASS |
            block::ICE => false,
            _ => true,
        }
    } else {
        false
    }
}
