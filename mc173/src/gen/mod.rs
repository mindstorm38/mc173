//! World generation module.

use std::sync::Arc;

use crate::source::{ChunkSource, ChunkSourceError};
use crate::world::ChunkSnapshot;
use crate::chunk::Chunk;

mod cave;

mod overworld;
pub use overworld::OverworldGenerator;


/// Chunk source for generating a world.
pub struct GeneratorChunkSource<G: ChunkGenerator> {
    /// The inner generator immutable structure shared between all workers.
    generator: Arc<G>,
    /// The owned cache for the generator.
    cache: G::Cache,
}

impl<G> GeneratorChunkSource<G>
where
    G: ChunkGenerator,
    G::Cache: Default,
{

    /// Create a new generator with it default cache, this generator can then be cloned
    /// if desired and the cache will remain shared between all generators.
    #[inline]
    pub fn new(generator: G) -> Self {
        Self {
            generator: Arc::new(generator),
            cache: Default::default(),
        }
    }

}

// Had to manually implement Clone because derive could not figure out how to do 
// with cache being clone or not.
impl<G> Clone for GeneratorChunkSource<G>
where
    G: ChunkGenerator,
    G::Cache: Clone
{

    fn clone(&self) -> Self {
        Self { 
            generator: Arc::clone(&self.generator), 
            cache: self.cache.clone(),
        }
    }

}

impl<G> ChunkSource for GeneratorChunkSource<G>
where
    G: ChunkGenerator,
{

    type LoadError = ();
    type SaveError = ();

    fn load(&mut self, cx: i32, cz: i32) -> Result<ChunkSnapshot, ChunkSourceError<Self::LoadError>> {

        let mut chunk = ChunkSnapshot::new(cx, cz);
        let chunk_access = Arc::get_mut(&mut chunk.chunk).unwrap();

        self.generator.generate(cx, cz, chunk_access, &mut self.cache);

        Ok(chunk)

    }

}


/// A trait common to all chunk generators, such generator can be used as a chunk source
/// through a [`GeneratorChunkSource`] object.
pub trait ChunkGenerator {

    /// Type of the cache that is only owned by a single worker, the generator itself
    /// should however be 
    type Cache;

    /// Generate the chunk terrain but do not populate it.
    fn generate(&self, cx: i32, cz: i32, chunk: &mut Chunk, cache: &mut Self::Cache);

}
