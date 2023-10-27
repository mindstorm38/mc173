//! Information about breaking abilities of tools and weapons items.

use crate::block::Material;
use crate::block;
use crate::item;


/// Check if an item (given its id) can break a block without speed penalties and loose
/// the items.
pub fn can_break(item_id: u16, block_id: u8) -> bool {
    
    match block_id {
        block::OBSIDIAN => matches!(item_id, 
            item::DIAMOND_PICKAXE),
        block::DIAMOND_ORE |
        block::DIAMOND_BLOCK |
        block::GOLD_ORE |
        block::GOLD_BLOCK |
        block::REDSTONE_ORE |
        block::REDSTONE_ORE_GLOWING => matches!(item_id, 
            item::DIAMOND_PICKAXE | 
            item::IRON_PICKAXE),
        block::IRON_ORE |
        block::IRON_BLOCK |
        block::LAPIS_ORE |
        block::LAPIS_BLOCK => matches!(item_id, 
            item::DIAMOND_PICKAXE | 
            item::IRON_PICKAXE | 
            item::STONE_PICKAXE),
        block::COBWEB => matches!(item_id, 
            item::SHEARS |
            item::DIAMOND_SWORD |
            item::IRON_SWORD |
            item::STONE_SWORD |
            item::GOLD_SWORD |
            item::WOOD_SWORD),
        block::SNOW |
        block::SNOW_BLOCK => matches!(item_id, 
            item::DIAMOND_SHOVEL | 
            item::IRON_SHOVEL | 
            item::STONE_SHOVEL |
            item::GOLD_SHOVEL |
            item::WOOD_SHOVEL),
        _ => {

            let material = block::from_id(block_id).material;
            if material.is_breakable_by_default() {
                return true;
            }

            match item_id {
                item::DIAMOND_PICKAXE |
                item::IRON_PICKAXE |
                item::STONE_PICKAXE |
                item::GOLD_PICKAXE |
                item::WOOD_PICKAXE => matches!(material, Material::Rock | Material::Iron),
                _ => false
            }

        }
    }

}

/// Get the speed multiplier for breaking a given block with a given item.
pub fn get_break_speed(item_id: u16, block_id: u8) -> f32 {
    
    const DIAMOND_SPEED: f32 = 8.0;
    const IRON_SPEED: f32 = 6.0;
    const STONE_SPEED: f32 = 4.0;
    const WOOD_SPEED: f32 = 2.0;
    const GOLD_SPEED: f32 = 12.0;

    match block_id {
        block::WOOD |
        block::BOOKSHELF |
        block::LOG |
        block::CHEST => {
            // Axe
            match item_id {
                item::DIAMOND_AXE => DIAMOND_SPEED,
                item::IRON_AXE => IRON_SPEED,
                item::STONE_AXE => STONE_SPEED,
                item::WOOD_AXE => WOOD_SPEED,
                item::GOLD_AXE => GOLD_SPEED,
                _ => 1.0,
            }
        }
        block::COBBLESTONE |
        block::SLAB |
        block::DOUBLE_SLAB |
        block::STONE |
        block::SANDSTONE |
        block::MOSSY_COBBLESTONE |
        block::IRON_ORE |
        block::IRON_BLOCK |
        block::COAL_ORE |
        block::DIAMOND_ORE |
        block::DIAMOND_BLOCK |
        block::ICE |
        block::NETHERRACK |
        block::LAPIS_ORE |
        block::LAPIS_BLOCK => {
            // Pickaxe
            match item_id {
                item::DIAMOND_PICKAXE => DIAMOND_SPEED,
                item::IRON_PICKAXE => IRON_SPEED,
                item::STONE_PICKAXE => STONE_SPEED,
                item::WOOD_PICKAXE => WOOD_SPEED,
                item::GOLD_PICKAXE => GOLD_SPEED,
                _ => 1.0,
            }
        }
        block::GRASS |
        block::DIRT |
        block::SAND |
        block::GRAVEL |
        block::SNOW |
        block::SNOW_BLOCK |
        block::CLAY |
        block::FARMLAND => {
            // Shovel
            match item_id {
                item::DIAMOND_SHOVEL => DIAMOND_SPEED,
                item::IRON_SHOVEL => IRON_SPEED,
                item::STONE_SHOVEL => STONE_SPEED,
                item::WOOD_SHOVEL => WOOD_SPEED,
                item::GOLD_SHOVEL => GOLD_SPEED,
                _ => 1.0,
            }
        }
        block::COBWEB => {
            match item_id {
                item::SHEARS |
                item::DIAMOND_SWORD |
                item::IRON_SWORD |
                item::STONE_SWORD |
                item::GOLD_SWORD |
                item::WOOD_SWORD => 15.0,
                _ => 1.0,
            }
        }
        block::LEAVES => {
            match item_id {
                item::SHEARS => 15.0,
                _ => 1.0,
            }
        }
        _ => match item_id {
            item::DIAMOND_SWORD |
            item::IRON_SWORD |
            item::STONE_SWORD |
            item::GOLD_SWORD |
            item::WOOD_SWORD => 1.5,
            _ => 1.0,
        }
    }

}


/// Get the minimum ticks duration required to break the block given its id.
pub fn get_break_duration(item_id: u16, block_id: u8, in_water: bool, on_ground: bool) -> f32 {

    // TODO: Maybe remove hardness from the block definition, because it's only used in
    // the game for break duration.

    let block = block::from_id(block_id);
    if block.hardness < 0.0 {
        f32::INFINITY
    } else {

        // The hardness value in the game is registered as ticks, with a multiplier
        // depending on the player's conditions and tools.

        if item::breaking::can_break(item_id, block_id) {

            let mut env_modifier = get_break_speed(item_id, block_id);

            if in_water {
                env_modifier /= 5.0;
            }

            if !on_ground {
                env_modifier /= 0.5;
            }
            
            block.hardness * 30.0 / env_modifier

        } else {
            block.hardness * 100.0
        }

    }

}
