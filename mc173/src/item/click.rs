//! Item interaction behaviors.

use glam::{IVec3, Vec2};

use crate::item::{self, ItemStack};
use crate::world::World;
use crate::util::Face;
use crate::block;


/// Use an item stack on a given block with a left click. This function returns the item 
/// stack after, if used, this may return an item stack with size of 0. If the given item
/// stack is empty, a block click is still processed, but the item will not be used.
pub fn click_at(world: &mut World, pos: IVec3, face: Face, stack: ItemStack, look: Vec2) -> Option<ItemStack> {

    // Only continue if position is legal.
    let (id, metadata) = world.block_and_metadata(pos)?;

    // If the block has been clicked, do not use the item.
    if block::click::click_at(world, pos, id, metadata) {
        return None;
    }

    // Do nothing if stack is empty.
    if stack.is_empty() {
        return None;
    }

    let success = match stack.id {
        0..=255 => place_block_at(world, pos, face, stack, id),
        item::WOOD_DOOR => place_door_at(world, pos, face, look, block::WOOD_DOOR),
        item::IRON_DOOR => place_door_at(world, pos, face, look,block::IRON_DOOR),
        _ => false
    };

    success.then_some(stack.with_size(stack.size - 1))

}


/// Place a block at the given block's face. The clicked block id is given
fn place_block_at(world: &mut World, mut pos: IVec3, face: Face, stack: ItemStack, id: u8) -> bool {

    let place_face;

    if id == block::SNOW {
        place_face = Face::NegY;
    } else {
        pos += face.delta();
        place_face = face.opposite();
    }

    let block_id = stack.id as u8;
    let block_metadata = stack.damage as u8;

    let block = block::from_id(block_id);
    if pos.y >= 127 && block.material.is_solid() {
        return false;
    } if !block::place::can_place_at(world, pos, place_face, block_id) {
        return false;
    }

    block::place::place_at(world, pos, place_face, block_id, block_metadata);
    true

}

/// Place a door item at given position.
fn place_door_at(world: &mut World, pos: IVec3, face: Face, look: Vec2, block_id: u8) -> bool {

    if face != Face::PosY {
        return false;
    }

    // Only positive face is allowed.
    let pos = pos + IVec3::Y;

    if pos.y >= 127 {
        return false;
    } else if !block::place::can_place_at(world, pos, face.opposite(), block_id) {
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
        block::place::is_block_at(world, left_pos, &[block_id]) || 
        block::place::is_block_at(world, left_pos + IVec3::Y, &[block_id]);

    let right_door = 
        block::place::is_block_at(world, right_pos, &[block_id]) || 
        block::place::is_block_at(world, right_pos + IVec3::Y, &[block_id]);

    if right_door && !left_door {
        flip = true;
    } else {

        let left_count = 
            block::place::is_block_opaque_at(world, left_pos) as u8 + 
            block::place::is_block_opaque_at(world, left_pos + IVec3::Y) as u8;
    
        let right_count = 
            block::place::is_block_opaque_at(world, right_pos) as u8 + 
            block::place::is_block_opaque_at(world, right_pos + IVec3::Y) as u8;

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

    block::door::set_upper(&mut metadata, false);
    block::door::set_face(&mut metadata, door_face);
    world.set_block_and_metadata(pos, block_id, metadata);

    block::door::set_upper(&mut metadata, true);
    world.set_block_and_metadata(pos + IVec3::Y, block_id, metadata);

    true

}
