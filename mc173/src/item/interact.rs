//! Item interaction behaviors.

use glam::IVec3;

use crate::block::{self, Face};
use crate::world::World;

use super::ItemStack;


/// Use an item stack on a given block. This function returns the item stack after, if 
/// used, this may return an item stack with size of 0.
pub fn use_on(world: &mut World, stack: ItemStack, pos: IVec3, face: Face) -> Option<ItemStack> {

    // Do nothing if stack is empty.
    if stack.is_empty() {
        return None;
    }

    // TODO: Try interaction with the block before using the item.

    match stack.id {
        0..=255 => use_block_on(world, stack, pos, face).then_some(stack.with_size(stack.size - 1)),
        _ => None
    }

}


fn use_block_on(world: &mut World, stack: ItemStack, mut pos: IVec3, mut face: Face) -> bool {

    // Only use the block item if chunk is valid.
    let Some((pos_block, _)) = world.block_and_metadata(pos) else { return false };

    if pos_block == block::SNOW {
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
