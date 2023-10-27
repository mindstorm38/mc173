//! Block breaking functions.

use glam::IVec3;

use crate::world::World;
use crate::block;


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
