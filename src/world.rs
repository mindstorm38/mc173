//! Data structure for storing a world (overworld or nether) at runtime.

use std::collections::HashMap;

use crate::chunk::Chunk;


/// Data structure for a whole world.
pub struct World {
    /// The abstract source for providing and saving the world.
    source: Box<dyn WorldSource>,
    /// Mapping of chunks to their coordinates.
    chunks: HashMap<(i32, i32), ChunkState>,
}

impl World {

    pub fn new(source: Box<dyn WorldSource>) -> Self {
        Self {
            source,
            chunks: HashMap::new(),
        }
    }

}


enum ChunkState {
    /// The chunk is present and contains valid data to interact with.
    Present(Box<Chunk>),
    /// The chunk has been requested to the chunk provider.
    Requested,
}


/// An abstract trait to implement on components that can provide chunks from a given
/// coordinate, this includes generators, deserializer and more.
pub trait WorldSource {

    /// Request a chunk to be loaded by this provider.
    fn request_chunk(&mut self, cx: i32, cz: i32);

    /// Poll a requested chunk.
    fn poll_chunk(&mut self) -> Option<(i32, i32, Box<Chunk>)>;

}
