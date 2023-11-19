//! Serialization and deserialization utilities for worlds, chunks and entities.


pub mod region;
pub mod nbt;

use std::io;
use std::path::PathBuf;
use std::sync::Arc;

use crate::source::{ChunkSource, ChunkSourceError};
use crate::world::ChunkSnapshot;

use self::region::{RegionDir, RegionError};


/// A chunk source for worlds that load and save chunk from/to region files.
pub struct RegionChunkSource {
    dir: RegionDir,
}

impl RegionChunkSource {

    /// Create a new chunk source for loading and saving chunk from/to the region 
    /// directory at the given path.
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { dir: RegionDir::new(path) }
    }

}


impl ChunkSource for RegionChunkSource {

    type LoadError = RegionError;
    type SaveError = RegionError;

    fn load_chunk(&mut self, cx: i32, cz: i32) -> Result<ChunkSnapshot, ChunkSourceError<Self::LoadError>> {

        // Get the region file but do not create it if not already existing, returning
        // unsupported if not existing.
        let region = match self.dir.ensure_region_file(cx, cz, false) {
            Ok(region) => region,
            Err(RegionError::Io(err)) if err.kind() == io::ErrorKind::NotFound => {
                return Err(ChunkSourceError::Unsupported);
            }
            Err(err) => return Err(ChunkSourceError::Other(err))
        };
        
        // Read the chunk, if it is empty then we return unsupported because we don't
        // have the chunk but it's not really an error.
        let chunk = match region.read_chunk(cx, cz) {
            Ok(chunk) => chunk,
            Err(RegionError::EmptyChunk) => return Err(ChunkSourceError::Unsupported),
            Err(err) => return Err(ChunkSourceError::Other(err)),
        };

        let root_raw = self::nbt::from_reader(chunk).unwrap();
        let root = root_raw.as_compound()
            .expect("chunk root should a compound");

        let level = root.get_compound("Level")
            .expect("chunk level should a compound");

        let actual_cx = level.get_int("xPos").unwrap();
        let actual_cz = level.get_int("zPos").unwrap();

        if (cx, cz) != (actual_cx, actual_cz) {
            panic!("incoherent chunk coordinates");
        }

        let mut snapshot = ChunkSnapshot::new(cx, cz);
        let chunk = Arc::get_mut(&mut snapshot.chunk).unwrap();

        let block = level.get_byte_array("Blocks").unwrap();
        chunk.block.copy_from_slice(block);
        let metadata = level.get_byte_array("Data").unwrap();
        chunk.metadata.inner.copy_from_slice(metadata);
        let block_light = level.get_byte_array("BlockLight").unwrap();
        chunk.block_light.inner.copy_from_slice(block_light);
        let sky_light = level.get_byte_array("SkyLight").unwrap();
        chunk.sky_light.inner.copy_from_slice(sky_light);
        let height_map = level.get_byte_array("HeightMap").unwrap();
        chunk.heigh_map.copy_from_slice(height_map);

        Ok(snapshot)

    }

}
