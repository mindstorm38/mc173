//! Block interaction behaviors, when clicking on the block.

use glam::IVec3;

use crate::world::World;
use crate::block;


/// Interact with a block at given position. This function returns true if an interaction
/// happened.
pub fn use_at(world: &mut World, pos: IVec3) -> bool {
    
    // Only continue if position is legal.
    let Some((id, metadata)) = world.get_block(pos) else { return false };

    match id {
        block::BUTTON => use_button(world, pos, metadata),
        block::LEVER => use_lever(world, pos, metadata),
        block::TRAPDOOR => use_trapdoor(world, pos, metadata),
        block::IRON_DOOR => true,
        block::WOOD_DOOR => use_wood_door(world, pos, metadata),
        block::REPEATER |
        block::REPEATER_LIT => use_repeater(world, pos, id, metadata),
        _ => return false
    }

}

/// Interact with a button block.
fn use_button(world: &mut World, pos: IVec3, mut metadata: u8) -> bool {
    if !block::button::is_active(metadata) {
        block::button::set_active(&mut metadata, true);
        world.set_block_notify(pos, block::BUTTON, metadata);
        world.schedule_tick(pos, block::BUTTON, 20);
    }
    true
}

fn use_lever(world: &mut World, pos: IVec3, mut metadata: u8) -> bool {
    let active = block::lever::is_active(metadata);
    block::lever::set_active(&mut metadata, !active);
    world.set_block_notify(pos, block::LEVER, metadata);
    true
}

fn use_trapdoor(world: &mut World, pos: IVec3, mut metadata: u8) -> bool {
    let active = block::trapdoor::is_open(metadata);
    block::trapdoor::set_open(&mut metadata, !active);
    world.set_block_notify(pos, block::TRAPDOOR, metadata);
    true
}

fn use_wood_door(world: &mut World, pos: IVec3, mut metadata: u8) -> bool {

    if block::door::is_upper(metadata) {
        if let Some((block::WOOD_DOOR, metadata)) = world.get_block(pos - IVec3::Y) {
            use_wood_door(world, pos - IVec3::Y, metadata);
        }
    } else {

        let open = block::door::is_open(metadata);
        block::door::set_open(&mut metadata, !open);

        world.set_block_notify(pos, block::WOOD_DOOR, metadata);

        if let Some((block::WOOD_DOOR, _)) = world.get_block(pos + IVec3::Y) {
            block::door::set_upper(&mut metadata, true);
            world.set_block_notify(pos + IVec3::Y, block::WOOD_DOOR, metadata);
        }

    }

    true

}

fn use_repeater(world: &mut World, pos: IVec3, id: u8, mut metadata: u8) -> bool {
    let delay = block::repeater::get_delay(metadata);
    block::repeater::set_delay(&mut metadata, (delay + 1) % 4);
    world.set_block_notify(pos, id, metadata);
    true
}
