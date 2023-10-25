//! Block interaction behaviors, when clicking on the block.

use glam::IVec3;

use crate::world::World;
use crate::block;


/// Interact with a block at given position. This function returns true if an interaction
/// happened.
pub fn use_at(world: &mut World, pos: IVec3, id: u8, metadata: u8) -> bool {
    match id {
        block::BUTTON => use_button(world, pos, metadata),
        block::LEVER => use_lever(world, pos, metadata),
        block::TRAPDOOR => use_trapdoor(world, pos, metadata),
        block::IRON_DOOR => true,
        block::WOOD_DOOR => use_wood_door(world, pos, metadata),
        _ => return false
    }
}

/// Interact with a button block.
fn use_button(world: &mut World, pos: IVec3, mut metadata: u8) -> bool {
    
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

fn use_lever(world: &mut World, pos: IVec3, mut metadata: u8) -> bool {
    let active = block::lever::is_active(metadata);
    block::lever::set_active(&mut metadata, !active);
    world.set_block_and_metadata(pos, block::LEVER, metadata);
    true
}

fn use_trapdoor(world: &mut World, pos: IVec3, mut metadata: u8) -> bool {
    let active = block::trapdoor::is_open(metadata);
    block::trapdoor::set_open(&mut metadata, !active);
    world.set_block_and_metadata(pos, block::TRAPDOOR, metadata);
    true
}

fn use_wood_door(world: &mut World, pos: IVec3, mut metadata: u8) -> bool {

    if block::door::is_upper(metadata) {
        if let Some((block::WOOD_DOOR, metadata)) = world.block_and_metadata(pos - IVec3::Y) {
            use_wood_door(world, pos - IVec3::Y, metadata);
        }
    } else {

        let open = block::door::is_open(metadata);
        block::door::set_open(&mut metadata, !open);

        world.set_block_and_metadata(pos, block::WOOD_DOOR, metadata);

        if let Some((block::WOOD_DOOR, _)) = world.block_and_metadata(pos + IVec3::Y) {
            block::door::set_upper(&mut metadata, true);
            world.set_block_and_metadata(pos + IVec3::Y, block::WOOD_DOOR, metadata);
        }

    }

    true

}
