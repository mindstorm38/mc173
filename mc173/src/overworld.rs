//! The overworld chunk source.

use glam::{IVec3, DVec3};

use crate::chunk::{Chunk, CHUNK_WIDTH};
use crate::world::{World, Dimension};
use crate::block::{STONE, GRASS};
use crate::entity::PigEntity;


pub fn new_overworld() -> World {

    let mut world = World::new(Dimension::Overworld);

    for cx in -10..10 {
        for cz in -10..10 {
            world.insert_chunk(cx, cz, new_overworld_chunk(64));
        }
    }

    // for _ in 0..4 {
    //     world.spawn_entity(PigEntity::new(DVec3::new(0.0, 70.0, 0.0)));
    // }

    for x in -3..3 {
        for z in -3..3 {
            world.set_block_and_metadata(IVec3::new(x, 63, z), 0, 0);
        }
    }

    world.set_spawn_position(DVec3::new(0.0, 66.0, 0.0));

    world

}


/// A temporary development function to generate a new flat overworld chunk.
pub fn new_overworld_chunk(height: i32) -> Box<Chunk> {
    let mut chunk = Chunk::new();
    chunk.fill_block_and_metadata(IVec3::new(0, 0, 0), IVec3::new(CHUNK_WIDTH as _, height - 3, CHUNK_WIDTH as _), STONE, 0);
    chunk.fill_block_and_metadata(IVec3::new(0, height - 3, 0), IVec3::new(CHUNK_WIDTH as _, 3, CHUNK_WIDTH as _), GRASS, 0);
    chunk
}
