//! A chunk storing blocks and other entities, optimized for runtime performance.

use std::io::{self, Write};

use smallvec::SmallVec;

use crate::block::AIR;


pub const CHUNK_WIDTH: usize = 16;
pub const CHUNK_HEIGHT: usize = 128;
pub const CHUNK_SIZE: usize = CHUNK_HEIGHT * CHUNK_WIDTH * CHUNK_WIDTH;


/// Calculate the index in the chunk's arrays for the given chunk-local position. This
/// is the same layout used by Minecraft's code `_xxx xzzz zyyy yyyy`.
#[inline]
fn calc_index(x: usize, y: usize, z: usize) -> usize {
    debug_assert!(x < CHUNK_WIDTH && z < CHUNK_WIDTH && y < CHUNK_HEIGHT);
    (x << 11) | (z << 7) | (y << 0)
}

/// Calculate the chunk position corresponding to the given block position. 
/// This also returns chunk-local coordinates in this chunk.
#[inline]
pub fn calc_chunk_pos(x: i32, z: i32) -> (i32, i32) {
    (x / CHUNK_WIDTH as i32, z / CHUNK_WIDTH as i32)
}


/// Data structure storing every chunk-local data, chunks are a world subdivision of 
/// 16x16x256 blocks.
pub struct Chunk {
    /// The numeric identifier of the block.
    block: ChunkByteArray,
    /// Four byte metadata for each block.
    metadata: ChunkNibbleArray,
    /// Block list level for each block.
    block_light: ChunkNibbleArray,
    /// Sky light level for each block.
    sky_light: ChunkNibbleArray,
    /// Because we store all chunks in boxes, we use a small vec to inline some entities
    /// into the storage to avoid double indirection for simple cases.
    entities: SmallVec<[usize; 8]>,
}

impl Chunk {

    /// Create a new empty chunk, full of air blocks.
    pub fn new() -> Box<Self> {
        Box::new(Self {
            block: [AIR; CHUNK_SIZE],
            metadata: ChunkNibbleArray::new(0),
            block_light: ChunkNibbleArray::new(15),
            sky_light: ChunkNibbleArray::new(0),
            entities: SmallVec::new(),
        })
    }

    /// Get block id at the given chunk-local position.
    pub fn block(&self, x: usize, y: usize, z: usize) -> u8 {
        self.block[calc_index(x, y, z)]
    }

    /// Get block metadata at the given chunk-local position.
    pub fn metadata(&self, x: usize, y: usize, z: usize) -> u8 {
        self.metadata.get(calc_index(x, y, z))
    }

    /// Get block light level at the given chunk-local position.
    pub fn block_light(&self, x: usize, y: usize, z: usize) -> u8 {
        self.block_light.get(calc_index(x, y, z))
    }

    /// Get sky light level at the given chunk-local position.
    pub fn sky_light(&self, x: usize, y: usize, z: usize) -> u8 {
        self.sky_light.get(calc_index(x, y, z))
    }

    /// Set block id at the given chunk-local position.
    pub fn set_block(&mut self, x: usize, y: usize, z: usize, id: u8) {
        self.block[calc_index(x, y, z)] = id;
    }

    /// Set block metadata at the given chunk-local position.
    pub fn set_metadata(&mut self, x: usize, y: usize, z: usize, metadata: u8) {
        self.metadata.set(calc_index(x, y, z), metadata)
    }

    /// Get block light level at the given chunk-local position.
    pub fn set_block_light(&mut self, x: usize, y: usize, z: usize, level: u8) {
        self.block_light.set(calc_index(x, y, z), level);
    }

    /// Get sky light level at the given chunk-local position.
    pub fn set_sky_light(&mut self, x: usize, y: usize, z: usize, level: u8) {
        self.sky_light.set(calc_index(x, y, z), level);
    }

    /// Fill the given chunk area with given block id and metadata.
    pub fn fill_block_and_metadata(&mut self, 
        x: usize, y: usize, z: usize,
        x_size: usize, y_size: usize, z_size: usize,
        id: u8, metadata: u8
    ) {

        for x in x..x + x_size {
            for z in z..z + z_size {

                let mut index = calc_index(x, y, z);

                for _ in y..y + y_size {

                    self.block[index] = id;
                    self.metadata.set(index, metadata);

                    // Increment Y component.
                    index += 1;

                }

            }
        }

    }

    /// Write the chunk's data to the given writer.
    pub fn write_data_to(&self, mut writer: impl Write) -> io::Result<()> {
        writer.write_all(&self.block)?;
        writer.write_all(&self.metadata.inner)?;
        writer.write_all(&self.block_light.inner)?;
        writer.write_all(&self.sky_light.inner)?;
        Ok(())
    }

    /// Add an entity to this chunk, you must ensure that this entity is not already in
    /// the chunk.
    pub fn add_entity(&mut self, entity_index: usize) {
        self.entities.push(entity_index);
    }

    /// Remove an entity from this chunk, you must ensure that it already exists in this
    /// chunk.
    pub fn remove_entity(&mut self, entity_index: usize) {
        let position = self.entities.iter().position(|&idx| idx == entity_index).unwrap();
        self.entities.swap_remove(position);
    }

    pub fn replace_entity(&mut self, old_entity_index: usize, new_entity_index: usize) {
        let position = self.entities.iter().position(|&idx| idx == old_entity_index).unwrap();
        self.entities[position] = new_entity_index;
    }

    pub fn entities(&self) -> &[usize] {
        &self.entities
    }

}

/// Type alias for a chunk array that stores `u8 * CHUNK_SIZE` values.
type ChunkByteArray = [u8; CHUNK_SIZE];

/// Special arrays for chunks that stores `u4 * CHUNK_SIZE` values.
struct ChunkNibbleArray {
    inner: [u8; CHUNK_SIZE / 2]
}

impl ChunkNibbleArray {

    const fn new(init: u8) -> Self {
        debug_assert!(init <= 0x0F);
        let init = init << 4 | init;
        Self { inner: [init; CHUNK_SIZE / 2] }
    }

    fn get(&self, index: usize) -> u8 {
        let slot = self.inner[index >> 1];
        if index & 1 == 0 {
            slot & 0x0F
        } else {
            (slot & 0xF0) >> 4
        }
    }

    fn set(&mut self, index: usize, value: u8) {
        debug_assert!(value <= 0x0F);
        let slot = &mut self.inner[index >> 1];
        if index & 1 == 0 {
            *slot = (*slot & 0xF0) | value;
        } else {
            *slot = (*slot & 0x0F) | (value << 4);
        }
    }

}
