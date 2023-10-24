//! Block ticking behavior.

use glam::IVec3;

use crate::world::World;
use crate::block;


/// Tick the block at the given position, this tick has been scheduled in the world.
pub fn tick_at(world: &mut World, pos: IVec3, id: u8, metadata: u8) {
    match id {
        block::BUTTON => tick_button(world, pos, metadata),
        _ => {}
    }
}

/// Tick a button block, this is used to deactivate the button after 20 ticks.
fn tick_button(world: &mut World, pos: IVec3, mut metadata: u8) {
    if block::button::is_active(metadata) {
        block::button::set_active(&mut metadata, false);
        world.set_block_and_metadata(pos, block::BUTTON, metadata);
        // TODO: Notify neighbor change for the pos and its faced block.
    }
}
