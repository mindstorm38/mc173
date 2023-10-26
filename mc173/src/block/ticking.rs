//! Block ticking behavior.

use glam::IVec3;

use crate::world::World;
use crate::block;


/// Tick the block at the given position, this tick has been scheduled in the world.
pub fn tick_at(world: &mut World, pos: IVec3, id: u8, metadata: u8) {
    match id {
        block::BUTTON => tick_button(world, pos, metadata),
        block::REPEATER => tick_repeater(world, pos, metadata, false),
        block::REPEATER_LIT => tick_repeater(world, pos, metadata, true),
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

fn tick_repeater(world: &mut World, pos: IVec3, metadata: u8, lit: bool) {

    let face = block::repeater::get_face(metadata);
    let delay = block::repeater::get_delay_ticks(metadata);
    let back_powered = block::powering::get_passive_power_from(world, pos - face.delta(), face) != 0;

    if lit && !back_powered {
        world.set_block_and_metadata(pos, block::REPEATER, metadata);
    } else if !lit {
        world.set_block_and_metadata(pos, block::REPEATER_LIT, metadata);
        if !back_powered {
            world.schedule_tick(pos, block::REPEATER_LIT, delay);
        }
    }

    // Notify the powered block in front of.
    block::notifying::notify_around(world, pos);
    // Also notify the powered block.
    block::notifying::notify_around(world, pos + face.delta());

}
