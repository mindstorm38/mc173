//! The overworld chunk source.

use glam::IVec3;

use crate::chunk::{Chunk, CHUNK_WIDTH};
use crate::block::{STONE, GRASS};

/// A temporary development function to generate a new flat overworld chunk.
pub fn new_overworld_chunk() -> Box<Chunk> {
    let mut chunk = Chunk::new();
    chunk.fill_block_and_metadata(IVec3::new(0, 0, 0), IVec3::new(CHUNK_WIDTH as _, 61, CHUNK_WIDTH as _), STONE, 0);
    chunk.fill_block_and_metadata(IVec3::new(0, 61, 0), IVec3::new(CHUNK_WIDTH as _, 3, CHUNK_WIDTH as _), GRASS, 0);
    chunk
}
