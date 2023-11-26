//! World generation module.

use crate::source::{ChunkSource, ChunkSourceError};
use crate::world::ChunkSnapshot;


/// Chunk source for generating a world.
pub struct GeneratorChunkSource {

}

impl ChunkSource for GeneratorChunkSource {

    type LoadError = ();
    type SaveError = ();

    fn load_chunk(&mut self, cx: i32, cz: i32) -> Result<ChunkSnapshot, ChunkSourceError<Self::LoadError>> {
        let _ = (cx, cz);
        Err(ChunkSourceError::Unsupported)
    }

}
