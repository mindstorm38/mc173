//! A chunk storing blocks and other entities, optimized for runtime performance.

use std::io::{self, Write};

use glam::{IVec3, DVec3};

use crate::block::AIR;


/// Chunk size in both X and Z coordinates.
pub const CHUNK_WIDTH: usize = 16;
/// Chunk height.
pub const CHUNK_HEIGHT: usize = 128;
/// Internal chunk 2D size, in number of columns per chunk.
const CHUNK_2D_SIZE: usize = CHUNK_WIDTH * CHUNK_WIDTH;
/// Internal chunk 3D size, in number of block per chunk.
const CHUNK_3D_SIZE: usize = CHUNK_HEIGHT * CHUNK_2D_SIZE;

/// Calculate the index in the chunk's arrays for the given position (local or not). This
/// is the same layout used by Minecraft's code `_xxx xzzz zyyy yyyy`. Only firsts 
/// relevant bits are taken in each coordinate component.
#[inline]
fn calc_3d_index(pos: IVec3) -> usize {
    debug_assert!(pos.y >= 0 && pos.y < CHUNK_HEIGHT as i32);
    let x = pos.x as u32 & 0b1111;
    let z = pos.z as u32 & 0b1111;
    let y = pos.y as u32 & 0b1111111;
    ((x << 11) | (z << 7) | (y << 0)) as usize
}

/// Calculate the index in the chunk's 2D arrays for the given position (local or not).
/// Y position is ignored.
#[inline]
fn calc_2d_index(pos: IVec3) -> usize {
    let x = pos.x as u32 & 0b1111;
    let z = pos.z as u32 & 0b1111;
    ((z << 4) | (x << 0)) as usize
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
    (pos.x >> 4, pos.z >> 4)
}

/// Calculate the chunk position where the given entity should be cached.
#[inline]
pub fn calc_entity_chunk_pos(pos: DVec3) -> (i32, i32) {
    // NOTE: Using unchecked because entities don't have limit for Y value.
    calc_chunk_pos_unchecked(pos.floor().as_ivec3())
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
    ///  The height map
    heigh_map: ChunkHeightMap,
}

impl Chunk {

    /// Create a new empty chunk, full of air blocks.
    pub fn new() -> Box<Self> {
        Box::new(Self {
            block: [AIR; CHUNK_3D_SIZE],
            metadata: ChunkNibbleArray::new(0),
            block_light: ChunkNibbleArray::new(0),
            sky_light: ChunkNibbleArray::new(15),
            heigh_map: [0; CHUNK_2D_SIZE],
        })
    }

    /// Get block id and metadata at the given global position (rebased to chunk-local).
    /// Panics if Y component of the position is not between 0 and 128 (excluded).
    #[inline]
    pub fn get_block(&self, pos: IVec3) -> (u8, u8) {
        let index = calc_3d_index(pos);
        (self.block[index], self.metadata.get(index))
    }

    /// Set block id and metadata at the given global position (rebased to chunk-local).
    /// Panics if Y component of the position is not between 0 and 128 (excluded).
    #[inline]
    pub fn set_block(&mut self, pos: IVec3, id: u8, metadata: u8) {
        let index = calc_3d_index(pos);
        self.block[index] = id;
        self.metadata.set(index, metadata);
    }

    /// Get block light level at the given global position (rebased to chunk-local).
    /// Panics if Y component of the position is not between 0 and 128 (excluded).
    #[inline]
    pub fn get_block_light(&self, pos: IVec3) -> u8 {
        self.block_light.get(calc_3d_index(pos))
    }

    /// Get block light level at the given global position (rebased to chunk-local).
    /// Panics if Y component of the position is not between 0 and 128 (excluded).
    #[inline]
    pub fn set_block_light(&mut self, pos: IVec3, level: u8) {
        self.block_light.set(calc_3d_index(pos), level);
    }

    /// Get sky light level at the given global position (rebased to chunk-local).
    /// Panics if Y component of the position is not between 0 and 128 (excluded).
    #[inline]
    pub fn get_sky_light(&self, pos: IVec3) -> u8 {
        self.sky_light.get(calc_3d_index(pos))
    }

    /// Get sky light level at the given global position (rebased to chunk-local).
    /// Panics if Y component of the position is not between 0 and 128 (excluded).
    #[inline]
    pub fn set_sky_light(&mut self, pos: IVec3, level: u8) {
        self.sky_light.set(calc_3d_index(pos), level);
    }

    /// Get the height at the given position, the Y component is ignored.
    /// 
    /// The height value corresponds to the Y value of the first block above the column
    /// with full sky light.
    #[inline]
    pub fn get_height(&self, pos: IVec3) -> u8 {
        self.heigh_map[calc_2d_index(pos)]
    }

    /// Set the height at the given position, the Y component is ignored. 
    /// 
    /// The height value corresponds to the Y value of the first block above the column
    /// with full sky light.
    #[inline]
    pub fn set_height(&mut self, pos: IVec3, height: u8) {
        self.heigh_map[calc_2d_index(pos)] = height;
    }

    /// Fill the given chunk area with given block id and metadata.
    /// Panics if Y component of the position is not between 0 and 128 (excluded).
    pub fn fill_block(&mut self, start: IVec3, size: IVec3, id: u8, metadata: u8) {

        for x in start.x..start.x + size.x {
            for z in start.z..start.z + size.z {
                let mut index = calc_3d_index(IVec3::new(x, start.y, z));
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

}

/// Type alias for a chunk array that stores `u8 * CHUNK_2D_SIZE` values.
type ChunkHeightMap = [u8; CHUNK_2D_SIZE];

/// Type alias for a chunk array that stores `u8 * CHUNK_3D_SIZE` values.
type ChunkByteArray = [u8; CHUNK_3D_SIZE];

/// Special arrays for chunks that stores `u4 * CHUNK_3D_SIZE` values.
struct ChunkNibbleArray {
    inner: [u8; CHUNK_3D_SIZE / 2]
}

impl ChunkNibbleArray {

    const fn new(init: u8) -> Self {
        debug_assert!(init <= 0x0F);
        let init = init << 4 | init;
        Self { inner: [init; CHUNK_3D_SIZE / 2] }
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
