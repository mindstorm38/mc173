//! The overworld chunk source.

use crate::chunk::{Chunk, CHUNK_WIDTH};
use crate::driver::{Source, Event};
use crate::block::{STONE, GRASS};
use crate::world::World;


/// The source for generating an overworld dimension.
pub struct OverworldSource {
    chunks: Vec<(i32, i32, Box<Chunk>)>,
}

impl OverworldSource {

    pub fn new() -> Self {
        Self {
            chunks: Vec::new(),
        }
    }

}


impl Source for OverworldSource {

    fn tick(&mut self, world: &mut World, events: &mut Vec<Event>) {
        for (cx, cz, chunk) in self.chunks.drain(..) {
            world.insert_chunk(cx, cz, chunk);
            events.push(Event::ChunkLoaded { cx, cz });
        }
    }

    fn request_chunk(&mut self, cx: i32, cz: i32) {
        
        let mut chunk = Chunk::new();
        chunk.fill_block_and_metadata(0, 0, 0, CHUNK_WIDTH, 61, CHUNK_WIDTH, STONE, 0);
        chunk.fill_block_and_metadata(0, 61, 0, CHUNK_WIDTH, 3, CHUNK_WIDTH, GRASS, 0);

        self.chunks.push((cx, cz, chunk));

    }

}
