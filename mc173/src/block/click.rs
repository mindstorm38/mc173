//! Block interaction behaviors, when clicking on the block.

use glam::IVec3;

use crate::world::World;
use crate::block;


/// Interact with a block at given position. This function returns true if an interaction
/// happened.
pub fn click_at(world: &mut World, pos: IVec3, id: u8, metadata: u8) -> bool {
    match id {
        block::BUTTON => click_button(world, pos, metadata),
        block::LEVER => click_lever(world, pos, metadata),
        _ => return false
    }
}

/// Interact with a button block.
fn click_button(world: &mut World, pos: IVec3, mut metadata: u8) -> bool {
    
    if block::button::is_active(metadata) {
        return true;
    }

    block::button::set_active(&mut metadata, true);

    world.set_block_and_metadata(pos, block::BUTTON, metadata);
    // TODO: Notify neighbor changes.
    // TODO: Notify neighbor change for face block (block::button::get_face(metadata)).

    world.schedule_tick(pos, block::BUTTON, 20);
    
    true

}

fn click_lever(world: &mut World, pos: IVec3, mut metadata: u8) -> bool {
    let active = block::lever::is_active(metadata);
    block::lever::set_active(&mut metadata, !active);
    world.set_block_and_metadata(pos, block::LEVER, metadata);
    true
}
