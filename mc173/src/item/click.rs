//! Item interaction behaviors.

use glam::IVec3;

use crate::block::{self, Face};
use crate::world::World;

use super::ItemStack;


/// Use an item stack on a given block. This function returns the item stack after, if 
/// used, this may return an item stack with size of 0. If the given item stack is empty,
/// a block click is still processed, but the item will not be used.
pub fn click_at(world: &mut World, stack: ItemStack, pos: IVec3, face: Face) -> Option<ItemStack> {

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
        0..=255 => place_block_at(world, stack, pos, id, metadata, face).then_some(stack.with_size(stack.size - 1)),
        _ => None
    }

}


/// Place a block at the given block's face.
fn place_block_at(world: &mut World, stack: ItemStack, mut pos: IVec3, id: u8, metadata: u8, mut face: Face) -> bool {

    if id == block::SNOW {
        face = Face::NegY;
    } else {
        pos += face.delta();
    }

    let block = block::from_id(stack.id as u8);
    if pos.y >= 127 && block.material.is_solid() {
        return false;
    }

    println!("placing {stack:?} at {pos}");

    // TODO: Do not ignore blocking entities.
    // TODO: Check that block can be placed on wall for example.
    world.set_block_and_metadata(pos, stack.id as u8, stack.damage as u8);
    true

}
