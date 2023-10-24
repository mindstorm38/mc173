//! Item interaction behaviors.

use glam::IVec3;

use crate::block::{self, Face};
use crate::world::World;

use super::ItemStack;


/// Use an item stack on a given block with a left click. This function returns the item 
/// stack after, if used, this may return an item stack with size of 0. If the given item
/// stack is empty, a block click is still processed, but the item will not be used.
pub fn click_at(world: &mut World, pos: IVec3, face: Face, stack: ItemStack) -> Option<ItemStack> {

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

    match stack.id {
        0..=255 => place_block_at(world, pos, face, stack, id)
            .then_some(stack.with_size(stack.size - 1)),
        _ => None
    }

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
