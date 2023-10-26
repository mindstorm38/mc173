//! Block breaking functions.

use glam::IVec3;

use crate::world::World;
use crate::block;
use crate::item;


/// Break a block naturally and drop its items. This returns true if successful, false
/// if the chunk/pos was not valid. It also notifies blocks around.
pub fn break_at(world: &mut World, pos: IVec3) -> Option<(u8, u8)> {
    let (prev_id, prev_metadata) = remove_at(world, pos)?;
    block::dropping::drop_at(world, pos, prev_id, prev_metadata, 1.0);
    Some((prev_id, prev_metadata))
}


/// Remove a block but do not drop its items. It handles notification of surrounding 
/// blocks and also handles particular blocks destroy actions.
pub fn remove_at(world: &mut World, pos: IVec3) -> Option<(u8, u8)> {

    let (id, metadata) = world.set_block_and_metadata(pos, 0, 0)?;
    block::notifying::notify_around(world, pos);

    match id {
        block::LEVER if block::lever::is_active(metadata) => {
            if let Some((face, _)) = block::lever::get_face(metadata) {
                block::notifying::notify_around(world, pos + face.delta());
            }
        }
        block::BUTTON if block::button::is_active(metadata) => {
            if let Some(face) = block::button::get_face(metadata) {
                block::notifying::notify_around(world, pos + face.delta());
            }
        }
        block::REPEATER_LIT => {
            block::notifying::notify_around(world, pos + block::repeater::get_face(metadata).delta());
        }
        _ => {}
    }

    Some((id, metadata))

}


/// Get the minimum ticks duration required to break the block given its id.
pub fn get_break_duration(block_id: u8, item_id: u16, in_water: bool, on_ground: bool) -> f32 {

    // TODO: Maybe remove hardness from the block definition, because it's only used in
    // the game for break duration.

    let block = block::from_id(block_id);
    if block.hardness < 0.0 {
        f32::INFINITY
    } else {

        // The hardness value in the game is registered as ticks, with a multiplier
        // depending on the player's conditions and tools.

        if item::breaking::can_break(item_id, block_id) {

            let mut env_modifier = item::breaking::get_break_speed(item_id, block_id);

            if in_water {
                env_modifier /= 5.0;
            }

            if !on_ground {
                env_modifier /= 0.5;
            }
            
            block.hardness * 30.0 / env_modifier

        } else {
            block.hardness * 100.0
        }

    }

}
