//! Item interaction behaviors.

use glam::{IVec3, Vec2};

use crate::item::{self, ItemStack};
use crate::world::World;
use crate::util::Face;
use crate::block;


/// Use an item stack on a given block with a left click. This function returns the item 
/// stack after, if used, this may return an item stack with size of 0. The face is where
/// the click has hit on the target block.
pub fn use_at(world: &mut World, pos: IVec3, face: Face, look: Vec2, stack: ItemStack) -> Option<ItemStack> {

    if stack.is_empty() {
        return None;
    }
    
    let success = match stack.id {
        0 => false,
        1..=255 => place_block_at(world, pos, face, look, stack.id as u8, stack.damage as u8),
        item::SUGAR_CANES => place_block_at(world, pos, face, look, block::SUGAR_CANES, 0),
        item::CAKE => place_block_at(world, pos, face, look, block::CAKE, 0),
        item::REPEATER => place_block_at(world, pos, face, look, block::REPEATER, 0),
        item::REDSTONE => place_block_at(world, pos, face, look, block::REDSTONE, 0),
        item::WOOD_DOOR => place_door_at(world, pos, face, look, block::WOOD_DOOR),
        item::IRON_DOOR => place_door_at(world, pos, face, look, block::IRON_DOOR),
        item::BED => place_bed_at(world, pos, face, look),
        _ => false
    };

    success.then_some(stack.with_size(stack.size - 1))

}


/// Place a block toward the given face. This is used for single blocks, multi blocks
/// are handled apart by other functions that do not rely on the block placing logic.
fn place_block_at(world: &mut World, mut pos: IVec3, mut face: Face, look: Vec2, id: u8, metadata: u8) -> bool {

    if let Some((block::SNOW, _)) = world.block(pos) {
        // If a block is placed by clicking on a snow block, replace that snow block.
        face = Face::NegY;
    } else {
        // Get position of the block facing the clicked face.
        pos += face.delta();
        // The block is oriented toward that clicked face.
        face = face.opposite();
    }

    // Some block have special facing when placed.
    match id {
        block::WOOD_STAIR | block::COBBLESTONE_STAIR |
        block::REPEATER | block::REPEATER_LIT => {
            face = Face::from_yaw(look.x);
        }
        block::DISPENSER |
        block::FURNACE | block::FURNACE_LIT |
        block::PUMPKIN | block::PUMPKIN_LIT => {
            face = Face::from_yaw(look.x).opposite();
        }
        block::PISTON => {
            face = Face::from_look(look.x, look.y).opposite();
        }
        _ => {}
    }

    if pos.y >= 127 && block::from_id(id).material.is_solid() {
        return false;
    } if !block::placing::can_place_at(world, pos, face, id) {
        return false;
    }

    block::placing::place_at(world, pos, face, id, metadata);
    true

}

/// Place a door item at given position.
fn place_door_at(world: &mut World, mut pos: IVec3, face: Face, look: Vec2, block_id: u8) -> bool {

    if face != Face::PosY {
        return false;
    } else {
        pos += IVec3::Y;
    }

    if pos.y >= 127 {
        return false;
    } else if !block::placing::can_place_at(world, pos, face.opposite(), block_id) {
        return false;
    }

    // The door face the opposite of the placer's look.
    let mut door_face = Face::from_yaw(look.x).opposite();
    let mut flip = false;
    
    // Here we count the block on the left and right (from the door face), this will
    // change the default orientation of the door.
    let left_pos = pos + door_face.rotate_left().delta();
    let right_pos = pos + door_face.rotate_right().delta();

    let left_door = 
        block::placing::is_block_at(world, left_pos, &[block_id]) || 
        block::placing::is_block_at(world, left_pos + IVec3::Y, &[block_id]);

    let right_door = 
        block::placing::is_block_at(world, right_pos, &[block_id]) || 
        block::placing::is_block_at(world, right_pos + IVec3::Y, &[block_id]);

    if right_door && !left_door {
        flip = true;
    } else {

        let left_count = 
            block::placing::is_block_opaque_at(world, left_pos) as u8 + 
            block::placing::is_block_opaque_at(world, left_pos + IVec3::Y) as u8;
    
        let right_count = 
            block::placing::is_block_opaque_at(world, right_pos) as u8 + 
            block::placing::is_block_opaque_at(world, right_pos + IVec3::Y) as u8;

        if left_count > right_count {
            flip = true;
        }

    }

    let mut metadata = 0;

    // To flip the door, we rotate it left and open it by default.
    if flip {
        block::door::set_open(&mut metadata, true);
        door_face = door_face.rotate_left();
    }

    block::door::set_face(&mut metadata, door_face);
    world.set_block(pos, block_id, metadata);

    block::door::set_upper(&mut metadata, true);
    world.set_block(pos + IVec3::Y, block_id, metadata);

    true

}

fn place_bed_at(world: &mut World, mut pos: IVec3, face: Face, look: Vec2) -> bool {

    if face != Face::PosY {
        return false;
    } else {
        pos += IVec3::Y;
    }

    let bed_face = Face::from_yaw(look.x);
    let head_pos = pos + bed_face.delta();

    let mut metadata = 0;
    block::bed::set_face(&mut metadata, bed_face);

    if block::placing::is_block_at(world, pos, &[block::AIR]) && 
        block::placing::is_block_at(world, head_pos, &[block::AIR]) &&
        block::placing::is_block_opaque_at(world, pos - IVec3::Y) &&
        block::placing::is_block_opaque_at(world, head_pos - IVec3::Y) {
        
        world.set_block(pos, block::BED, metadata);

        block::bed::set_head(&mut metadata, true);
        world.set_block(head_pos, block::BED, metadata);

    }

    true 

}
