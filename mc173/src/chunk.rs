//! A chunk storing block and light data of a world, optimized for runtime performance. 
//! This  module only provides low-level data structures, refer to the [`mc173::world`] 
//! module for world manipulation methods.

use std::io::{self, Write};
use std::sync::Arc;

use glam::{IVec3, DVec3};

use crate::biome::Biome;
use crate::block;


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
#[derive(Clone)]
pub struct Chunk {
    /// The numeric identifier of the block.
    pub block: ChunkArray3<u8>,
    /// Four byte metadata for each block.
    pub metadata: ChunkNibbleArray3,
    /// Block list level for each block.
    pub block_light: ChunkNibbleArray3,
    /// Sky light level for each block.
    pub sky_light: ChunkNibbleArray3,
    ///  The height map.
    pub height: ChunkArray2<u8>,
    /// The biome map, this map is not actually saved nor sent to the client. It is
    /// internally used by this implementation to really split the chunk generation from
    /// the running world. The Notchian server is different because the mob spawning
    /// algorithms requires the biome map to be generated at runtime. This also explains
    /// why we can use a Rust enumeration for this one, and not raw value, because we
    /// don't need to deserialize it and therefore don't risk any unwanted value.
    pub biome: ChunkArray2<Biome>,
}

impl Chunk {

    /// Create a new empty chunk, full of air blocks. All block light is zero and all sky
    /// light is 15. This constructor directly returns an arc chunk to ensure that no 
    /// useless copy will be done, and also because it make no sense to hold this 
    /// structure on stack.
    /// 
    /// The chunk is specifically returned in a Atomic Reference-Counted container in 
    /// order to be used as some kind of Clone-On-Write container (through the method
    /// [`Arc::make_mut`]), this is especially useful when dealing with zero-copy 
    /// asynchronous chunk saving.
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            block: [block::AIR; CHUNK_3D_SIZE],
            metadata: ChunkNibbleArray3::new(0),
            block_light: ChunkNibbleArray3::new(0),
            sky_light: ChunkNibbleArray3::new(15),
            height: [0; CHUNK_2D_SIZE],
            biome: [Biome::Void; CHUNK_2D_SIZE],
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
    pub fn set_block_light(&mut self, pos: IVec3, light: u8) {
        self.block_light.set(calc_3d_index(pos), light);
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
    pub fn set_sky_light(&mut self, pos: IVec3, light: u8) {
        self.sky_light.set(calc_3d_index(pos), light);
    }

    /// Get the height at the given position, the Y component is ignored.
    /// 
    /// The height value corresponds to the Y value of the first block above the column
    /// with full sky light.
    #[inline]
    pub fn get_height(&self, pos: IVec3) -> u8 {
        self.height[calc_2d_index(pos)]
    }

    /// Set the height at the given position, the Y component is ignored. 
    /// 
    /// The height value corresponds to the Y value of the first block above the column
    /// with full sky light.
    #[inline]
    pub fn set_height(&mut self, pos: IVec3, height: u8) {
        self.height[calc_2d_index(pos)] = height;
    }

    /// Get the biome at the given position, the Y component is ignored.
    #[inline]
    pub fn get_biome(&self, pos: IVec3) -> Biome {
        self.biome[calc_2d_index(pos)]
    }

    /// Set the biome at the given position, the Y component is ignored.
    #[inline]
    pub fn set_biome(&mut self, pos: IVec3, biome: Biome) {
        self.biome[calc_2d_index(pos)] = biome;
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

    /// Fill the given chunk area with given block and sky light values.
    /// Panics if Y component of the position is not between 0 and 128 (excluded).
    pub fn fill_light(&mut self, start: IVec3, size: IVec3, block_light: u8, sky_light: u8)  {
        for x in start.x..start.x + size.x {
            for z in start.z..start.z + size.z {
                let mut index = calc_3d_index(IVec3::new(x, start.y, z));
                for _ in start.y..start.y + size.y {
                    self.block_light.set(index, block_light);
                    self.sky_light.set(index, sky_light);
                    // Increment Y component.
                    index += 1;
                }
            }
        }
    }

    /// Recompute the whole height map based on all block in the chunk. This also reset
    /// all sky light values to the right values each columns. Note that skylight is not
    /// propagated and therefore the updates should be scheduled manually when chunk is
    /// added to a world. Block light is not touched.
    pub fn recompute_height(&mut self) {
        for x in 0..CHUNK_WIDTH {
            for z in 0..CHUNK_WIDTH {

                let mut sky_light = 15u8;
                for y in (0..CHUNK_HEIGHT).rev() {
                    
                    let pos = IVec3::new(x as i32, y as i32, z as i32);
                    let index_3d = calc_3d_index(pos);
                    let id = self.block[index_3d];

                    if sky_light != 0 {
                        let opacity = block::material::get_light_opacity(id);
                        if sky_light == 15 && opacity != 0 {
                            // We are currently above height, but the current block will
                            // block the light and therefore change set the height to the
                            // block above.
                            // NOTE: Cast is safe because Y is in range.
                            self.height[calc_2d_index(pos)] = y as u8 + 1;
                        }
                        sky_light = sky_light.saturating_sub(block::material::get_light_opacity(id));
                    }

                    self.sky_light.set(index_3d, sky_light);

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
pub type ChunkArray2<T> = [T; CHUNK_2D_SIZE];

/// Type alias for a chunk array that stores `u8 * CHUNK_3D_SIZE` values.
pub type ChunkArray3<T> = [T; CHUNK_3D_SIZE];

/// Special arrays for chunks that stores `u4 * CHUNK_3D_SIZE` values.
#[derive(Clone)]
pub struct ChunkNibbleArray3 {
    pub inner: [u8; CHUNK_3D_SIZE / 2]
}

impl ChunkNibbleArray3 {

    pub const fn new(init: u8) -> Self {
        debug_assert!(init <= 0x0F);
        let init = init << 4 | init;
        Self { inner: [init; CHUNK_3D_SIZE / 2] }
    }

    #[inline]
    pub fn get(&self, index: usize) -> u8 {
        let slot = self.inner[index >> 1];
        if index & 1 == 0 {
            slot & 0x0F
        } else {
            (slot & 0xF0) >> 4
        }
    }

    #[inline]
    pub fn set(&mut self, index: usize, value: u8) {
        debug_assert!(value <= 0x0F);
        let slot = &mut self.inner[index >> 1];
        if index & 1 == 0 {
            *slot = (*slot & 0xF0) | value;
        } else {
            *slot = (*slot & 0x0F) | (value << 4);
        }
    }

}
