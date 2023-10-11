//! Data structure for storing a world (overworld or nether) at runtime.

use std::collections::HashMap;

use glam::{IVec3, DVec3};

use crate::chunk::{Chunk, calc_chunk_pos};


/// Calculate the chunk position corresponding to the given block position. 
/// This also returns chunk-local coordinates in this chunk.
#[inline]
pub fn calc_entity_chunk_pos(pos: DVec3) -> (i32, i32) {
    calc_chunk_pos(pos.x as i32, pos.z as i32)
}


/// Data structure for a whole world.
pub struct World {
    /// The dimension
    dimension: Dimension,
    /// The spawn position.
    spawn_pos: IVec3,
    /// The world time, increasing at each tick.
    time: u64,
    /// Mapping of chunks to their coordinates.
    chunks: HashMap<(i32, i32), Box<Chunk>>,
}

impl World {

    pub fn new(dimension: Dimension) -> Self {
        Self {
            chunks: HashMap::new(),
            dimension,
            spawn_pos: IVec3::ZERO,
            time: 0,
        }
    }

    pub fn dimension(&self) -> Dimension {
        self.dimension
    }

    pub fn spawn_pos(&self) -> IVec3 {
        self.spawn_pos
    }

    pub fn set_spawn_pos(&mut self, pos: IVec3) {
        self.spawn_pos = pos;
    }

    pub fn time(&self) -> u64 {
        self.time
    }

    pub fn set_time(&mut self, time: u64) {
        self.time = time;
    }

    pub fn chunk(&self, cx: i32, cz: i32) -> Option<&Chunk> {
        self.chunks.get(&(cx, cz)).map(|c| &**c)
    }

    pub fn chunk_mut(&mut self, cx: i32, cz: i32) -> Option<&mut Chunk> {
        self.chunks.get_mut(&(cx, cz)).map(|c| &mut **c)
    }

    pub fn insert_chunk(&mut self, cx: i32, cz: i32, chunk: Box<Chunk>) {
        self.chunks.insert((cx, cz), chunk);
        // TODO: Check for orphan entities.
    }

    pub fn remove_chunk(&mut self, cx: i32, cz: i32) -> Option<Box<Chunk>> {
        self.chunks.remove(&(cx, cz))
        // TODO: Unlink entities.
    }

}

/// Types of dimensions, used for ambient effects in the world.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Dimension {
    /// The overworld dimension with a blue sky and day cycles.
    Overworld,
    /// The creepy nether dimension.
    Nether,
}
