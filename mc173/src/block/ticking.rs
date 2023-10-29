//! Block ticking behavior.

use glam::IVec3;

use crate::util::{Face, FaceSet};
use crate::world::{World, Dimension};
use crate::block;


/// Tick the block at the given position, this tick has been scheduled in the world.
pub fn tick_at(world: &mut World, pos: IVec3, id: u8, metadata: u8) {
    match id {
        block::BUTTON => tick_button(world, pos, metadata),
        block::REPEATER => tick_repeater(world, pos, metadata, false),
        block::REPEATER_LIT => tick_repeater(world, pos, metadata, true),
        block::REDSTONE_TORCH => tick_redstone_torch(world, pos, metadata, false),
        block::REDSTONE_TORCH_LIT => tick_redstone_torch(world, pos, metadata, true),
        block::WATER_MOVING => tick_fluid_moving(world, pos, metadata, block::WATER_MOVING, block::WATER_STILL),
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

/// Tick a moving fluid block.
fn tick_fluid_moving(world: &mut World, pos: IVec3, mut metadata: u8, moving_id: u8, still_id: u8) {

    // Default distance to decrement on each block unit.
    let dist_drop = match moving_id {
        block::LAVA_MOVING if world.dimension() != Dimension::Nether => 2,
        _ => 1,
    };

    let tick_interval = match moving_id {
        block::LAVA_MOVING => 30,
        _ => 5,
    };

    // The id below is used many time after, so we query it here.
    let (below_id, below_metadata) = world.block(pos - IVec3::Y).unwrap_or((block::AIR, 0));

    // Update this fluid state.
    if !block::fluid::is_source(metadata) {

        // Default to 8, so if no fluid block is found around, fluid will disappear.
        let mut shortest_dist = 8;
        let mut sources_around = 0u8;

        for face in [Face::NegX, Face::PosX, Face::NegZ, Face::PosZ] {
            if let Some((face_id, face_metadata)) = world.block(pos + face.delta()) {
                // Only if this block is of the same type.
                if face_id == moving_id || face_id == still_id {
                    let face_dist = block::fluid::get_actual_distance(face_metadata);
                    shortest_dist = shortest_dist.min(face_dist);
                    if block::fluid::is_source(face_metadata) {
                        sources_around += 1;
                    }
                }
            }
        }

        let mut new_metadata = shortest_dist + dist_drop;
        if new_metadata > 7 {
            // Just mark that the metadata is invalid, fluid should disappear.
            new_metadata = 0xFF;
        }

        // If the top block on top is the same fluid, this become a falling state fluid.
        if let Some((above_id, above_metadata)) = world.block(pos + IVec3::Y) {
            if above_id == moving_id || above_id == still_id {
                // Copy the above metadata but force falling state.
                new_metadata = above_metadata;
                block::fluid::set_falling(&mut new_metadata, true);
            }
        }

        // Infinite water sources!
        if sources_around >= 2 && moving_id == block::WATER_MOVING {
            if block::from_id(below_id).material.is_solid() {
                block::fluid::set_source(&mut new_metadata);
            } else if below_id == moving_id || below_id == still_id {
                if block::fluid::is_source(below_metadata) {
                    block::fluid::set_source(&mut new_metadata);
                }
            }
        }

        // TODO: Weird lava stuff.

        if new_metadata != metadata {
            metadata = new_metadata;
            if new_metadata == 0xFF {
                world.set_block(pos, block::AIR, 0);
            } else {
                world.set_block(pos, moving_id, new_metadata);
                world.schedule_tick(pos, moving_id, tick_interval);
            }
        } else {
            world.set_block(pos, still_id, metadata);
        }

    } else {
        world.set_block(pos, still_id, metadata);
    }

    // In each case we modified the block, so we notify blocks around.
    block::notifying::notify_around(world, pos);

    // The block has been removed, don't propagate it.
    if metadata == 0xFF {
        return;
    }

    let blocked_below = block::fluid::is_fluid_blocked(below_id);
    if !block::fluid::is_fluid_block(below_id) && !blocked_below {
        // The block below is not a fluid block and do not block fluids, the fluid below
        // is set to a falling version of the current block.
        block::fluid::set_falling(&mut metadata, true);
        world.set_block(pos - IVec3::Y, moving_id, metadata);
        block::notifying::notify_around(world, pos - IVec3::Y);
        block::notifying::notify_at(world, pos - IVec3::Y);
    } else if block::fluid::is_source(metadata) || blocked_below {

        // The block is a source or is blocked below, we spread it horizontally.
        // let open_faces = FaceSet::new();
        // for face in [Face::NegX, Face::PosX, Face::NegZ, Face::PosZ] {
        //     if let Some((face_id, face_metadata)) = world.block(pos + face.delta()) {
        //         if !block::fluid::is_fluid_blocked(face_id) {
        //             if block::fluid::is_source(face_metadata) || (face_id != moving_id && face_id != still_id) {

        //             }
        //         }
        //     }
        // }

        let new_dist = block::fluid::get_actual_distance(metadata) + dist_drop;
        if new_dist > 7 {
            return;
        }

        for face in [Face::NegX, Face::PosX, Face::NegZ, Face::PosZ] {
            let face_pos = pos + face.delta();
            if let Some((face_id, _)) = world.block(face_pos) {
                if !block::fluid::is_fluid_block(face_id) && !block::fluid::is_fluid_blocked(face_id) {
                    // TODO: Break only for water.
                    block::breaking::break_at(world, face_pos);
                    world.set_block(face_pos, moving_id, new_dist);
                    world.schedule_tick(face_pos, moving_id, tick_interval);
                    block::notifying::notify_around(world, face_pos);
                    block::notifying::notify_at(world, face_pos);
                }
            }
        }

    }

}
