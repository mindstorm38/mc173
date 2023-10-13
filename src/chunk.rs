//! A chunk storing blocks and other entities, optimized for runtime performance.

use std::io::{self, Write};

use smallvec::SmallVec;
use glam::IVec3;

use crate::block::AIR;


/// Chunk size in both X and Z coordinates.
pub const CHUNK_WIDTH: usize = 16;
/// Chunk height.
pub const CHUNK_HEIGHT: usize = 128;
/// Internal chunk size, in number of elements per chunk.
const CHUNK_SIZE: usize = CHUNK_HEIGHT * CHUNK_WIDTH * CHUNK_WIDTH;


/// Calculate the index in the chunk's arrays for the given chunk-local position. This
/// is the same layout used by Minecraft's code `_xxx xzzz zyyy yyyy`. Only firsts 
/// relevant bits are taken in each coordinate component.
#[inline]
fn calc_index(pos: IVec3) -> usize {
    debug_assert!(pos.y >= 0 && pos.y < CHUNK_HEIGHT as i32);
    let x = pos.x as usize & 0b1111;
    let z = pos.z as usize & 0b1111;
    let y = pos.y as usize & 0b1111111;
    (x << 11) | (z << 7) | (y << 0)
}

/// Calculate the chunk position corresponding to the given block position. This returns
/// no position if the Y coordinate is invalid.
#[inline]
pub fn calc_chunk_pos(pos: IVec3) -> Option<(i32, i32)> {
    if pos.y < 0 || pos.y >= CHUNK_HEIGHT as i32 {
        None
    } else {
        Some(calc_chunk_pos_unchecked(pos))
    }
}

/// Calculate the chunk position corresponding to the given block position. The Y 
/// coordinate is ignored, so it may be invalid.
#[inline]
pub fn calc_chunk_pos_unchecked(pos: IVec3) -> (i32, i32) {
    (pos.x / CHUNK_WIDTH as i32, pos.z / CHUNK_WIDTH as i32)
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

    /// Get block id at the given global position (rebased to chunk-local).
    /// Panics if Y component of the position is not between 0 and 128 (excluded).
    #[inline]
    pub fn block(&self, pos: IVec3) -> u8 {
        self.block[calc_index(pos)]
    }

    /// Get block metadata at the given global position (rebased to chunk-local).
    /// Panics if Y component of the position is not between 0 and 128 (excluded).
    #[inline]
    pub fn metadata(&self, pos: IVec3) -> u8 {
        self.metadata.get(calc_index(pos))
    }

    /// Get block id and metadata at the given global position (rebased to chunk-local).
    /// Panics if Y component of the position is not between 0 and 128 (excluded).
    #[inline]
    pub fn block_and_metadata(&self, pos: IVec3) -> (u8, u8) {
        let index = calc_index(pos);
        (self.block[index], self.metadata.get(index))
    }

    /// Get block light level at the given global position (rebased to chunk-local).
    /// Panics if Y component of the position is not between 0 and 128 (excluded).
    #[inline]
    pub fn block_light(&self, pos: IVec3) -> u8 {
        self.block_light.get(calc_index(pos))
    }

    /// Get sky light level at the given global position (rebased to chunk-local).
    /// Panics if Y component of the position is not between 0 and 128 (excluded).
    #[inline]
    pub fn sky_light(&self, pos: IVec3) -> u8 {
        self.sky_light.get(calc_index(pos))
    }

    /// Set block id at the given global position (rebased to chunk-local).
    /// Panics if Y component of the position is not between 0 and 128 (excluded).
    #[inline]
    pub fn set_block(&mut self, pos: IVec3, block: u8) {
        self.block[calc_index(pos)] = block;
    }

    /// Set block id and metadata at the given global position (rebased to chunk-local).
    /// Panics if Y component of the position is not between 0 and 128 (excluded).
    #[inline]
    pub fn set_block_and_metadata(&mut self, pos: IVec3, block: u8, metadata: u8) {
        let index = calc_index(pos);
        self.block[index] = block;
        self.metadata.set(index, metadata);
    }

    /// Set block metadata at the given global position (rebased to chunk-local).
    /// Panics if Y component of the position is not between 0 and 128 (excluded).
    #[inline]
    pub fn set_metadata(&mut self, pos: IVec3, metadata: u8) {
        self.metadata.set(calc_index(pos), metadata);
    }

    /// Get block light level at the given global position (rebased to chunk-local).
    /// Panics if Y component of the position is not between 0 and 128 (excluded).
    #[inline]
    pub fn set_block_light(&mut self, pos: IVec3, level: u8) {
        self.block_light.set(calc_index(pos), level);
    }

    /// Get sky light level at the given global position (rebased to chunk-local).
    /// Panics if Y component of the position is not between 0 and 128 (excluded).
    #[inline]
    pub fn set_sky_light(&mut self, pos: IVec3, level: u8) {
        self.sky_light.set(calc_index(pos), level);
    }

    /// Fill the given chunk area with given block id and metadata.
    /// Panics if Y component of the position is not between 0 and 128 (excluded).
    pub fn fill_block_and_metadata(&mut self, 
        start: IVec3,
        size: IVec3,
        id: u8, metadata: u8
    ) {

        for x in start.x..start.x + size.x {
            for z in start.z..start.z + size.z {
                let mut index = calc_index(IVec3::new(x, start.y, z));
                for _ in start.y..start.y + size.y {

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

    #[inline]
    fn get(&self, index: usize) -> u8 {
        let slot = self.inner[index >> 1];
        if index & 1 == 0 {
            slot & 0x0F
        } else {
            (slot & 0xF0) >> 4
        }
    }

    #[inline]
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
