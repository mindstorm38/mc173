//! Block breaking functions.

use glam::IVec3;

use crate::world::World;
use crate::block;


/// Break a block naturally and drop its items. This returns true if successful, false
/// if the chunk/pos was not valid. It also notifies blocks around.
pub fn break_at(world: &mut World, pos: IVec3) -> Option<(u8, u8)> {
    let (prev_id, prev_metadata) = world.set_block_notify(pos, block::AIR, 0)?;
    world.spawn_block_loot(pos, prev_id, prev_metadata, 1.0);
    Some((prev_id, prev_metadata))
}


/// Get the break hardness of a block, the block hardness is a value that define the time
/// a player need to hit a block before breaking. When the player's tool is able to break
/// the block, the hardness is multiplied by 1.5 seconds, but 5.0 seconds when not able.
pub fn get_hardness(id: u8) -> f32 {
    match id {
        block::LEAVES |
        block::BED |
        block::SNOW_BLOCK => 0.2,
        block::GLASS |
        block::GLOWSTONE => 0.3,
        block::LADDER |
        block::CACTUS |
        block::NETHERRACK => 0.4,
        block::DIRT |
        block::SAND |
        block::STICKY_PISTON |
        block::PISTON |
        block::PISTON_EXT |
        block::LEVER |
        block::STONE_PRESSURE_PLATE |
        block::WOOD_PRESSURE_PLATE |
        block::BUTTON |
        block::ICE |
        block::SOULSAND |
        block::CAKE => 0.5,
        block::GRASS |
        block::GRAVEL |
        block::SPONGE |
        block::FARMLAND |
        block::CLAY => 0.6,
        block::POWERED_RAIL |
        block::DETECTOR_RAIL |
        block::RAIL => 0.7,
        block::SANDSTONE |
        block::NOTE_BLOCK |
        block::WOOL => 0.8,
        block::STONE |
        block::BOOKSHELF => 1.5,
        block::COBBLESTONE |
        block::WOOD |
        block::LOG |
        block::DOUBLE_SLAB |
        block::SLAB |
        block::BRICK |
        block::MOSSY_COBBLESTONE |
        block::WOOD_STAIR |
        block::COBBLESTONE_STAIR |
        block::JUKEBOX |
        block::FENCE => 2.0,
        block::GOLD_ORE |
        block::IRON_ORE |
        block::COAL_ORE |
        block::LAPIS_ORE |
        block::LAPIS_BLOCK |
        block::GOLD_BLOCK |
        block::DIAMOND_ORE |
        block::WOOD_DOOR |
        block::REDSTONE_ORE |
        block::REDSTONE_ORE_LIT |
        block::TRAPDOOR => 3.0,
        block::DISPENSER |
        block::FURNACE |
        block::FURNACE_LIT => 3.5,
        block::COBWEB => 4.0,
        block::IRON_BLOCK |
        block::DIAMOND_BLOCK |
        block::IRON_DOOR |
        block::SPAWNER => 5.0,
        block::OBSIDIAN => 10.0,
        block::BEDROCK |
        block::PISTON_MOVING |
        block::PORTAL |
        block::WATER_MOVING |
        block::WATER_STILL |
        block::LAVA_MOVING |
        block::LAVA_STILL => f32::INFINITY,
        _ => 0.0,
    }
}
