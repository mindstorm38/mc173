//! The overworld chunk source.

use glam::{IVec3, DVec3};

use mc173::chunk::{Chunk, CHUNK_WIDTH};
use mc173::world::{World, Dimension};
use mc173::util::rand::JavaRandom;
use mc173::block;


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
    chunk.fill_block_and_metadata(IVec3::new(0, 0, 0), IVec3::new(CHUNK_WIDTH as _, height - 3, CHUNK_WIDTH as _), block::STONE, 0);
    chunk.fill_block_and_metadata(IVec3::new(0, height - 3, 0), IVec3::new(CHUNK_WIDTH as _, 2, CHUNK_WIDTH as _), block::DIRT, 0);
    chunk.fill_block_and_metadata(IVec3::new(0, height - 1, 0), IVec3::new(CHUNK_WIDTH as _, 1, CHUNK_WIDTH as _), block::GRASS, 0);
    
    let mut rand = JavaRandom::new_seeded();
    for _ in 0..16 {

        let pos = IVec3 {
            x: rand.next_int_bounded(16),
            y: height - 1,
            z: rand.next_int_bounded(16),
        };

        chunk.set_block(pos, block::WOOD);
        
    }
    
    chunk

}
