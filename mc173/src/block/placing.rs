//! Block placing functions.

use glam::IVec3;

use crate::world::World;
use crate::util::Face;
use crate::block;


/// This function checks if the given block id can be placed at a particular position in
/// the world, the given face indicates toward which face this block should be oriented.
pub fn can_place_at(world: &mut World, pos: IVec3, face: Face, id: u8) -> bool {
    let base = match id {
        block::BUTTON if face.is_y() => false,
        block::BUTTON => is_block_opaque_at(world, pos + face.delta()),
        block::LEVER if face == Face::PosY => false,
        block::LEVER => is_block_opaque_at(world, pos + face.delta()),
        block::LADDER => is_block_opaque_around(world, pos),
        block::TRAPDOOR if face.is_y() => false,
        block::TRAPDOOR => is_block_opaque_at(world, pos + face.delta()),
        block::PISTON_EXT |
        block::PISTON_MOVING => false,
        block::DEAD_BUSH => is_block_at(world, pos - IVec3::Y, &[block::SAND]),
        block::DANDELION |
        block::POPPY |
        block::SAPLING |
        block::TALL_GRASS => is_block_at(world, pos - IVec3::Y, &[block::GRASS, block::DIRT, block::FARMLAND]),
        block::WHEAT => is_block_at(world, pos - IVec3::Y, &[block::FARMLAND]),
        block::CACTUS => can_place_cactus_at(world, pos),
        block::SUGAR_CANES => true, // TODO:
        block::CAKE => is_block_solid_at(world, pos - IVec3::Y),
        block::CHEST => can_place_chest_at(world, pos),
        block::WOOD_DOOR |
        block::IRON_DOOR => can_place_door_at(world, pos),
        block::FENCE => is_block_at(world, pos - IVec3::Y, &[block::FENCE]) || is_block_solid_at(world, pos - IVec3::Y),
        block::FIRE => true, // TODO:
        block::TORCH |
        block::REDSTONE_TORCH |
        block::REDSTONE_TORCH_LIT => is_block_opaque_at(world, pos + face.delta()),
        // Common blocks that needs opaque block below.
        block::RED_MUSHROOM |
        block::BROWN_MUSHROOM |
        block::WOOD_PRESSURE_PLATE |
        block::STONE_PRESSURE_PLATE |
        block::PUMPKIN |
        block::PUMPKIN_LIT |
        block::RAIL | 
        block::POWERED_RAIL |
        block::DETECTOR_RAIL |
        block::REPEATER |
        block::REPEATER_LIT |
        block::REDSTONE |
        block::SNOW => is_block_opaque_at(world, pos - IVec3::Y),
        _ => true,
    };
    base && is_block_replaceable_at(world, pos)
}

fn can_place_cactus_at(world: &mut World, pos: IVec3) -> bool {
    for face in [Face::NegX, Face::PosX, Face::NegZ, Face::PosZ] {
        if is_block_solid_at(world, pos + face.delta()) {
            return false;
        }
    }
    is_block_at(world, pos - IVec3::Y, &[block::CACTUS, block::SAND])
}

fn can_place_chest_at(world: &mut World, pos: IVec3) -> bool {
    let mut found_single_chest = false;
    for face in [Face::NegX, Face::PosX, Face::NegZ, Face::PosZ] {
        // If block on this face is a chest, check if that block also has a chest.
        let neighbor_pos = pos + face.delta();
        if is_block_at(world, neighbor_pos, &[block::CHEST]) {
            // We can't put chest
            if found_single_chest {
                return false;
            }
            // Check if the chest we found isn't a double chest.
            for neighbor_face in [Face::NegX, Face::PosX, Face::NegZ, Face::PosZ] {
                // Do not check our potential position.
                if face != neighbor_face.opposite() {
                    if is_block_at(world, neighbor_pos + neighbor_face.delta(), &[block::CHEST]) {
                        return false; // The chest found already is double.
                    }
                }
            }
            // No other chest found, it's a single chest.
            found_single_chest = true;
        }
    }
    true
}

fn can_place_door_at(world: &mut World, pos: IVec3) -> bool {
    is_block_opaque_at(world, pos - IVec3::Y) && is_block_replaceable_at(world, pos + IVec3::Y)
}


/// Place the block at the given position in the world oriented toward given face. Note
/// that this function do not check if this is legal, it will do what's asked. Also, the
/// given metadata may be modified to account for the placement.
pub fn place_at(world: &mut World, pos: IVec3, face: Face, id: u8, metadata: u8) {

    match id {
        block::BUTTON => place_faced_at(world, pos, face, id, metadata, block::button::set_face),
        block::TRAPDOOR => place_faced_at(world, pos, face, id, metadata, block::trapdoor::set_face),
        block::PISTON => place_faced_at(world, pos, face, id, metadata, block::piston::set_face),
        block::WOOD_STAIR | 
        block::COBBLESTONE_STAIR => place_faced_at(world, pos, face, id, metadata, block::stair::set_face),
        block::REPEATER | 
        block::REPEATER_LIT => place_faced_at(world, pos, face, id, metadata, block::repeater::set_face),
        block::PUMPKIN | 
        block::PUMPKIN_LIT => place_faced_at(world, pos, face, id, metadata, block::pumpkin::set_face),
        block::FURNACE | 
        block::FURNACE_LIT |
        block::DISPENSER => place_faced_at(world, pos, face, id, metadata, block::common::set_horizontal_face),
        block::TORCH |
        block::REDSTONE_TORCH |
        block::REDSTONE_TORCH_LIT => place_faced_at(world, pos, face, id, metadata, block::torch::set_face),
        block::LEVER => place_lever_at(world, pos, face, metadata),
        block::LADDER => place_ladder_at(world, pos, face, metadata),
        _ => {
            world.set_block(pos, id, metadata);
        }
    }

    // Self-notifying blocks.
    match id {
        block::REDSTONE_TORCH |
        block::REDSTONE_TORCH_LIT |
        block::REPEATER |
        block::REPEATER_LIT |
        block::REDSTONE => block::notifying::notify_at(world, pos),
        _ => {}
    }

    block::notifying::notify_around(world, pos);
    
}

/// Generic function to place a block that has a basic facing function.
fn place_faced_at(world: &mut World, pos: IVec3, face: Face, id: u8, mut metadata: u8, func: impl FnOnce(&mut u8, Face)) {
    func(&mut metadata, face);
    world.set_block(pos, id, metadata);
}

fn place_lever_at(world: &mut World, pos: IVec3, face: Face, mut metadata: u8) {
    // When facing down, randomly pick the orientation.
    block::lever::set_face(&mut metadata, face, match face {
        Face::NegY => world.rand_mut().next_choice(&[Face::PosZ, Face::PosX]),
        _ => Face::PosY,
    });
    world.set_block(pos, block::LEVER, metadata);
}

fn place_ladder_at(world: &mut World, pos: IVec3, mut face: Face, mut metadata: u8) {
    // Privileging desired face, but if desired face cannot support a ladder.
    if face.is_y() || !is_block_opaque_at(world, pos + face.delta()) {
        // NOTE: Order is important for parity with client.
        for around_face in [Face::PosZ, Face::NegZ, Face::PosX, Face::NegX] {
            if is_block_opaque_at(world, pos + around_face.delta()) {
                face = around_face;
                break;
            }
        }
    }
    block::ladder::set_face(&mut metadata, face);
    world.set_block(pos, block::LADDER, metadata);
}


/// Check is there are at least one opaque block around horizontally.
pub fn is_block_opaque_around(world: &mut World, pos: IVec3) -> bool {
    for face in [Face::NegX, Face::PosX, Face::NegZ, Face::PosZ] {
        if is_block_opaque_at(world, pos + face.delta()) {
            return true;
        }
    }
    false
}

/// Return true if the block at given position can be replaced.
pub fn is_block_replaceable_at(world: &mut World, pos: IVec3) -> bool {
    if let Some((id, _)) = world.block(pos) {
        block::from_id(id).material.is_replaceable()
    } else {
        false
    }
}

/// Return true if the block at position is opaque.
pub fn is_block_opaque_at(world: &mut World, pos: IVec3) -> bool {
    if let Some((id, _)) = world.block(pos) {
        block::material::is_opaque_cube(id)
    } else {
        false
    }
}

/// Return true if the block at position is material solid.
pub fn is_block_solid_at(world: &mut World, pos: IVec3) -> bool {
    if let Some((id, _)) = world.block(pos) {
        block::from_id(id).material.is_solid()
    } else {
        false
    }
}

/// Return true if the block at given position is in the valid slice.
pub fn is_block_at(world: &mut World, pos: IVec3, valid: &[u8]) -> bool {
    if let Some((id, _)) = world.block(pos) {
        valid.iter().any(|&valid_id| valid_id == id)
    } else {
        false
    }
}
