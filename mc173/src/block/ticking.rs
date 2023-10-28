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
        block::REDSTONE_TORCH => tick_redstone_torch(world, pos, metadata, false),
        block::REDSTONE_TORCH_LIT => tick_redstone_torch(world, pos, metadata, true),
        _ => {}
    }
}

/// Tick a button block, this is used to deactivate the button after 20 ticks.
fn tick_button(world: &mut World, pos: IVec3, mut metadata: u8) {
    if block::button::is_active(metadata) {

        block::button::set_active(&mut metadata, false);
        world.set_block(pos, block::BUTTON, metadata);

        block::notifying::notify_around(world, pos);
        if let Some(face) = block::button::get_face(metadata) {
            block::notifying::notify_around(world, pos + face.delta());
        }

    }
}

fn tick_repeater(world: &mut World, pos: IVec3, metadata: u8, lit: bool) {

    let face = block::repeater::get_face(metadata);
    let delay = block::repeater::get_delay_ticks(metadata);
    let back_powered = block::powering::get_passive_power_from(world, pos - face.delta(), face) != 0;

    if lit && !back_powered {
        world.set_block(pos, block::REPEATER, metadata);
    } else if !lit {
        world.set_block(pos, block::REPEATER_LIT, metadata);
        if !back_powered {
            world.schedule_tick(pos, block::REPEATER_LIT, delay);
        }
    }

    // Notify the powered block in front of.
    block::notifying::notify_around(world, pos);
    // Also notify the powered block.
    block::notifying::notify_around(world, pos + face.delta());

}

fn tick_redstone_torch(world: &mut World, pos: IVec3, metadata: u8, lit: bool) {

    // TODO: Check torch burnout...

    let Some(torch_face) = block::torch::get_face(metadata) else { return };
    let powered = block::powering::get_passive_power_from(world, pos + torch_face.delta(), torch_face.opposite()) != 0;

    let mut notify = false;

    if lit {
        if powered {
            world.set_block(pos, block::REDSTONE_TORCH, metadata);
            notify = true;
        }
    } else {
        if !powered {
            world.set_block(pos, block::REDSTONE_TORCH_LIT, metadata);
            notify = true;
        }
    }

    if notify {
        block::notifying::notify_around(world, pos);
        // FIXME: Not in the Notchian server, don't understand?
        block::notifying::notify_around(world, pos + IVec3::Y);
    }

}
