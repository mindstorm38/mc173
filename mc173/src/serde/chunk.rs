//! Chunk serialization and deserialization from NBT compound.

use std::borrow::Cow;
use std::collections::HashMap;
use std::io::{Read, Write};
use std::sync::Arc;

use glam::IVec3;

use crate::block_entity::BlockEntity;
use crate::world::ChunkSnapshot;
use crate::entity_new::Entity;

use super::nbt::NbtError;

mod block_entity_nbt;
mod entity_kind_nbt;
mod item_stack_nbt;
mod entity_nbt;
mod bytes_nbt;
mod slot_nbt;


/// Deserialize a chunk and its components from the given reader that contains NBT.
pub fn from_reader(reader: impl Read) -> Result<ChunkSnapshot, NbtError> {
    
    let nbt: RootNbt = super::nbt::from_reader(reader)?;
    let mut snapshot = ChunkSnapshot::new(nbt.level.x, nbt.level.z);
    let chunk = Arc::get_mut(&mut snapshot.chunk).unwrap();
    
    // This is annoying to make so much copies but we have no choice for know because 
    // we have no deserializer that would directly puts data in the chunk allocation.
    chunk.block.copy_from_slice(&nbt.level.block);
    chunk.metadata.inner.copy_from_slice(&nbt.level.metadata);
    chunk.block_light.inner.copy_from_slice(&nbt.level.block_light);
    chunk.sky_light.inner.copy_from_slice(&nbt.level.sky_light);
    chunk.height.copy_from_slice(&nbt.level.height);

    // Just move the deserialized components.
    snapshot.entities = nbt.level.entities.into_owned();
    snapshot.block_entities = nbt.level.block_entities;

    Ok(snapshot)

}

/// Serialize a chunk and its components to a given writer as NBT.
pub fn to_writer(writer: impl Write, snapshot: &ChunkSnapshot) -> Result<(), NbtError> {
    super::nbt::to_writer(writer, &RootNbt {
        level: LevelNbt {
            x: snapshot.cx,
            z: snapshot.cz,
            populated: true,
            block: Cow::Borrowed(&snapshot.chunk.block),
            metadata: Cow::Borrowed(&snapshot.chunk.metadata.inner),
            block_light: Cow::Borrowed(&snapshot.chunk.block_light.inner),
            sky_light: Cow::Borrowed(&snapshot.chunk.sky_light.inner),
            height: Cow::Borrowed(&snapshot.chunk.height),
            entities: Cow::Borrowed(&snapshot.entities),
            block_entities: snapshot.block_entities.clone(), // FIXME: Terrible
        }
    })
}

/// NOTE: We should be really careful by using the proper type that Notchian client uses,
/// and remember that using unsigned type is just a bit cast from signed ones.
#[derive(Debug, serde::Serialize, serde::Deserialize)]
struct RootNbt<'a> {
    #[serde(rename = "Level")]
    level: LevelNbt<'a>,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
struct LevelNbt<'a> {
    #[serde(rename = "xPos")]
    x: i32,
    #[serde(rename = "zPos")]
    z: i32,
    #[serde(rename = "TerrainPopulated")]
    populated: bool,
    #[serde(rename = "Blocks", with = "bytes_nbt")]
    block: Cow<'a, [u8]>,
    #[serde(rename = "Data", with = "bytes_nbt")]
    metadata: Cow<'a, [u8]>,
    #[serde(rename = "BlockLight", with = "bytes_nbt")]
    block_light: Cow<'a, [u8]>,
    #[serde(rename = "SkyLight", with = "bytes_nbt")]
    sky_light: Cow<'a, [u8]>,
    #[serde(rename = "HeightMap", with = "bytes_nbt")]
    height: Cow<'a, [u8]>,
    #[serde(rename = "Entities", with = "entity_nbt")]
    entities: Cow<'a, [Box<Entity>]>,
    #[serde(rename = "TileEntities", with = "block_entity_nbt")]
    block_entities: HashMap<IVec3, Box<BlockEntity>>,
}
