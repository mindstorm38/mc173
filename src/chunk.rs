//! A chunk storing blocks and other entities, optimized for runtime performance.

use crate::block::AIR;


pub const CHUNK_SIZE: usize = 16;
pub const CHUNK_HEIGHT: usize = 256;


/// Data structure storing every chunk-local data, chunks are a world subdivision of 
/// 16x16x256 blocks.
pub struct Chunk {
    /// Blocks data of the chunk, array layout is x/z/y.
    blocks: [[[BlockState; CHUNK_HEIGHT]; CHUNK_SIZE]; CHUNK_SIZE],
}

impl Chunk {

    /// Create a new empty chunk, full of air blocks.
    pub fn new() -> Box<Self> {
        Box::new(Self {
            blocks: [[[BlockState { id: AIR, metadata: 0 }; CHUNK_HEIGHT]; CHUNK_SIZE]; CHUNK_SIZE],
        })
    }

    pub fn get(&self, x: usize, y: usize, z: usize) -> BlockState {
        self.blocks[x][z][y]
    }

    pub fn set(&mut self, x: usize, y: usize, z: usize, state: BlockState) {
        self.blocks[x][z][y] = state;
    }

}


/// A short structure describing the state of a block at given coordinates.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct BlockState {
    /// The numeric identifier of the block, the block properties can be retrieved from
    /// [`crate::block::BLOCKS`] array.
    pub id: u8,
    /// A byte of metadata for this block state, the usage vary depending on the block
    /// type, for example it can be used to describe wool color with the 4 least 
    /// significant bits, or water level.
    pub metadata: u8,
}
