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

/// Get the light opacity of a block given its id.
pub fn get_light_opacity(id: u8) -> u8 {
    match id {
        block::AIR => 0,
        block::LEAVES |
        block::COBWEB => 1,
        block::WATER_MOVING |
        block::WATER_STILL |
        block::ICE => 3,
        _ =>  if is_opaque_cube(id) { 255 } else { 0 },
    }
}

/// Get the light emission of a block given its id. The resulting value is between
/// 0 and 15 included, so it's basically a 4-bit unsigned integer.
pub fn get_light_emission(id: u8) -> u8 {
    match id {
        block::BROWN_MUSHROOM => 1,
        block::REDSTONE_TORCH_LIT => 7,
        block::REDSTONE_ORE_LIT |
        block::REPEATER_LIT => 9,
        block::PORTAL => 11,
        block::FURNACE_LIT => 13,
        block::TORCH => 14,
        block::LAVA_MOVING |
        block::LAVA_STILL |
        block::FIRE |
        block::GLOWSTONE |
        block::PUMPKIN_LIT => 15,
        _ => 0
    }
}

/// The block slipperiness for entities.
pub fn get_slipperiness(id: u8) -> f32 {
    match id {
        block::ICE => 0.95,
        _ => 0.6
    }
}
