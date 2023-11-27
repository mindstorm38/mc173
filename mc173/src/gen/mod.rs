//! World generation module.

use std::sync::Arc;

use crate::source::{ChunkSource, ChunkSourceError};
use crate::world::ChunkSnapshot;
use crate::chunk::Chunk;

mod overworld;
pub use overworld::OverworldGenerator;


/// Chunk source for generating a world.
pub struct GeneratorChunkSource<G> {
    /// The inner generator.
    generator: G,
}

impl<G: ChunkGenerator> GeneratorChunkSource<G> {

    #[inline]
    pub fn new(generator: G) -> Self {
        Self { generator, }
    }

}

impl<G: ChunkGenerator> ChunkSource for GeneratorChunkSource<G> {

    type LoadError = ();
    type SaveError = ();

    fn load(&mut self, cx: i32, cz: i32) -> Result<ChunkSnapshot, ChunkSourceError<Self::LoadError>> {

        let mut chunk = ChunkSnapshot::new(cx, cz);
        let chunk_access = Arc::get_mut(&mut chunk.chunk).unwrap();

        self.generator.generate(cx, cz, chunk_access);

        Ok(chunk)

    }

}


/// A trait common to all chunk generators, such generator can be used as a chunk source
/// through a [`GeneratorChunkSource`] object.
pub trait ChunkGenerator {

    /// Generate the chunk terrain but do not populate it.
    fn generate(&mut self, cx: i32, cz: i32, chunk: &mut Chunk);

}
