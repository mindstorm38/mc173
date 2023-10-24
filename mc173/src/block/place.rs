//! Block placing functions.

use glam::IVec3;

use crate::block::{self, Face};
use crate::world::World;


/// This function checks if the given block id can be placed at a particular position in
/// the world, the given face indicates on which face this block should be oriented.
pub fn can_place_at(world: &mut World, pos: IVec3, face: Face, id: u8) -> bool {
    let base = match id {
        block::BUTTON if face.is_y() => false,
        block::BUTTON => is_block_opaque_at(world, pos, face),
        block::LEVER if face == Face::PosY => false,
        block::LEVER => is_block_opaque_at(world, pos, face),
        _ => true,
    };
    base && is_block_replaceable_at(world, pos)
}

/// Place the block at the given position in the world oriented toward given face. Note
/// that this function do not check if this is legal, it will do what's asked. Also, the
/// given metadata may be modified to account for the placement.
pub fn place_at(world: &mut World, pos: IVec3, face: Face, id: u8, metadata: u8) {
    match id {
        block::BUTTON => place_button_at(world, pos, face, metadata),
        block::LEVER => place_lever_at(world, pos, face, metadata),
        _ => {
            world.set_block_and_metadata(pos, id, metadata);
        }
    }
}

fn place_button_at(world: &mut World, pos: IVec3, face: Face, mut metadata: u8) {
    block::button::set_face(&mut metadata, face);
    world.set_block_and_metadata(pos, block::BUTTON, metadata);
}

fn place_lever_at(world: &mut World, pos: IVec3, face: Face, mut metadata: u8) {
    // When facing down, randomly pick the orientation.
    block::lever::set_face(&mut metadata, face, match face {
        Face::NegY => world.rand_mut().next_choice(&[Face::PosZ, Face::PosX]),
        _ => Face::PosY,
    });
    world.set_block_and_metadata(pos, block::LEVER, metadata);
}

/// Return true if the block at given position + face is opaque.
fn is_block_opaque_at(world: &mut World, pos: IVec3, face: Face) -> bool {
    if let Some((id, _)) = world.block_and_metadata(pos + face.delta()) {
        let block = block::from_id(id);
        block.material.is_opaque()
    } else {
        false
    }
}

/// Return true if the block at given position can be replaced.
fn is_block_replaceable_at(world: &mut World, pos: IVec3) -> bool {
    if let Some((id, _)) = world.block_and_metadata(pos) {
        let block = block::from_id(id);
        block.material.is_replaceable()
    } else {
        false
    }
}
