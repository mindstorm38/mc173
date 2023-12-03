//! Chunk serialization and deserialization from NBT compound.

use std::io::{Read, Write};
use std::sync::Arc;

use glam::IVec3;

use crate::entity::{Entity, EntityKind, ProjectileEntity, LivingEntity};

use crate::util::Face;
use crate::world::ChunkSnapshot;
use crate::item::ItemStack;

use crate::block_entity::note_block::NoteBlockBlockEntity;
use crate::block_entity::dispenser::DispenserBlockEntity;
use crate::block_entity::furnace::FurnaceBlockEntity;
use crate::block_entity::jukebox::JukeboxBlockEntity;
use crate::block_entity::spawner::SpawnerBlockEntity;
use crate::block_entity::piston::PistonBlockEntity;
use crate::block_entity::chest::ChestBlockEntity;
use crate::block_entity::sign::SignBlockEntity;
use crate::block_entity::BlockEntity;

use super::nbt::NbtError;


pub fn from_reader(reader: impl Read) -> Result<ChunkSnapshot, NbtError> {
    
    let root = super::nbt::from_reader::<RootNbt>(reader)?;
    let mut snapshot = ChunkSnapshot::new(root.level.x, root.level.z);
    let chunk = Arc::get_mut(&mut snapshot.chunk).unwrap();
    
    chunk.block.copy_from_slice(&root.level.block);
    chunk.metadata.inner.copy_from_slice(&root.level.metadata);
    chunk.block_light.inner.copy_from_slice(&root.level.block_light);
    chunk.sky_light.inner.copy_from_slice(&root.level.sky_light);
    chunk.height.copy_from_slice(&root.level.height);

    println!("root: {:?}", root.level.block_entities);

    Ok(snapshot)

}


#[derive(Debug, serde::Serialize, serde::Deserialize)]
struct RootNbt {
    #[serde(rename = "Level")]
    level: LevelNbt,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
struct LevelNbt {
    #[serde(rename = "xPos")]
    x: i32,
    #[serde(rename = "zPos")]
    z: i32,
    #[serde(rename = "TerrainPopulated")]
    populated: bool,
    #[serde(rename = "Blocks", with = "serde_bytes")]
    block: Vec<u8>,
    #[serde(rename = "Data", with = "serde_bytes")]
    metadata: Vec<u8>,
    #[serde(rename = "BlockLight", with = "serde_bytes")]
    block_light: Vec<u8>,
    #[serde(rename = "SkyLight", with = "serde_bytes")]
    sky_light: Vec<u8>,
    #[serde(rename = "HeightMap", with = "serde_bytes")]
    height: Vec<u8>,
    #[serde(rename = "Entities")]
    entities: Vec<EntityNbt>,
    #[serde(rename = "TileEntities")]
    block_entities: Vec<BlockEntityNbt>,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
struct EntityNbt {
    #[serde(rename = "Pos")]
    pos: (f64, f64, f64),
    #[serde(rename = "Motion")]
    vel: (f64, f64, f64),
    #[serde(rename = "Rotation")]
    look: (f32, f32),
    #[serde(rename = "FallDistance", default)]
    fall_distance: f32,
    #[serde(rename = "Fire", default)]
    fire_ticks: i16,
    #[serde(rename = "Air", default)]
    air_ticks: i16,
    #[serde(rename = "OnGround", default)]
    on_ground: bool,
    id: String,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
struct BlockEntityNbt {
    x: i32,
    y: i32,
    z: i32,
    #[serde(flatten)]
    kind: BlockEntityKindNbt,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
#[serde(tag = "id")]
enum BlockEntityKindNbt {
    Chest {
        #[serde(rename = "Items")]
        inv: Vec<SlotItemStackNbt>,
    },
    Furnace {
        #[serde(rename = "Items")]
        inv: Vec<SlotItemStackNbt>,
    },
    #[serde(rename = "RecordPlayer")]
    Jukebox {
        #[serde(rename = "Record")]
        record: u32
    },
    #[serde(rename = "Trap")]
    Dispenser {
        #[serde(rename = "Items")]
        inv: Vec<SlotItemStackNbt>,
    },
    Sign {
        #[serde(rename = "Text1")]
        text1: String,
        #[serde(rename = "Text2")]
        text2: String,
        #[serde(rename = "Text3")]
        text3: String,
        #[serde(rename = "Text4")]
        text4: String,
    },
    #[serde(rename = "MobSpawner")]
    Spawner {
        #[serde(rename = "EntityId")]
        entity_id: String,
        #[serde(rename = "Delay")]
        remaining_ticks: u16,
    },
    #[serde(rename = "Music")]
    NoteBlock {
        note: u8,
    },
    Piston {
        #[serde(rename = "blockId")]
        id: u8,
        #[serde(rename = "blockData")]
        metadata: u8,
        facing: u8,
        progress: f32,
        extending: bool,
    }
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
struct SlotItemStackNbt {
    #[serde(rename = "Slot")]
    slot: u8,
    #[serde(flatten)]
    stack: ItemStackNbt,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
struct ItemStackNbt {
    id: u16,
    #[serde(rename = "Count")]
    size: u8,
    #[serde(rename = "Damage")]
    damage: u16,
}


// /// Read a chunk and all of its components from the given NBT compound.
// pub fn from_nbt(root: &Nbt, only_populated: bool) -> Result<ChunkSnapshot, ChunkError> {

//     let root = root.as_compound().ok_or(invalid_tag("/ not compound"))?;
//     let level = root.get_compound("Level").ok_or(invalid_tag("/Level not compound"))?;

//     // Directly abort if the chunk is not populated yet.
//     if only_populated && !level.get_boolean("TerrainPopulated").unwrap_or(true) {
//         return Err(ChunkError::NotPopulated);
//     }

//     let cx = level.get_int("xPos").ok_or(invalid_tag("/Level/xPos not int"))?;
//     let cz = level.get_int("zPos").ok_or(invalid_tag("/Level/zPos not int"))?;

//     let mut snapshot = ChunkSnapshot::new(cx, cz);
//     let chunk = Arc::get_mut(&mut snapshot.chunk).unwrap();

//     let block = level.get_byte_array("Blocks").ok_or(invalid_tag("/Level/Blocks not byte array"))?;
//     chunk.block.copy_from_slice(block);
//     let metadata = level.get_byte_array("Data").ok_or(invalid_tag("/Level/Data not byte array"))?;
//     chunk.metadata.inner.copy_from_slice(metadata);
//     let block_light = level.get_byte_array("BlockLight").ok_or(invalid_tag("/Level/BlockLight not byte array"))?;
//     chunk.block_light.inner.copy_from_slice(block_light);
//     let sky_light = level.get_byte_array("SkyLight").ok_or(invalid_tag("/Level/SkyLight not byte array"))?;
//     chunk.sky_light.inner.copy_from_slice(sky_light);
//     let height_map = level.get_byte_array("HeightMap").ok_or(invalid_tag("/Level/HeightMap not byte array"))?;
//     chunk.height.copy_from_slice(height_map);

//     let entities = level.get_list("Entities").ok_or(invalid_tag("/Level/Entities not list"))?;
//     for entity in entities {
//         snapshot.entities.push(entity_from_nbt(entity)?);
//     }

//     let block_entities = level.get_list("TileEntities").ok_or(invalid_tag("/Level/TileEntities not list"))?;
//     for block_entity in block_entities {
//         let (pos, block_entity) = block_entity_from_nbt(block_entity)?;
//         snapshot.block_entities.insert(pos, block_entity);
//     }

//     Ok(snapshot)

// }

// /// Decode an entity from NBT.
// pub fn entity_from_nbt(root: &Nbt) -> Result<Box<Entity>, ChunkError> {

//     let root = root.as_compound().ok_or(invalid_tag("/ not compound"))?;
//     let entity_id = root.get_string("id").ok_or(invalid_tag("/id not string"))?;

//     let mut entity = entity_kind_from_id(entity_id)?.new_default();

//     let base = entity.base_mut();

//     let pos_list = root.get_list("Pos").ok_or(invalid_tag("/Pos not list"))?;
//     base.pos.x = pos_list[0].as_double().unwrap();
//     base.pos.y = pos_list[1].as_double().unwrap();
//     base.pos.z = pos_list[2].as_double().unwrap();

//     let motion_list = root.get_list("Motion").ok_or(invalid_tag("/Motion not list"))?;
//     base.vel.x = motion_list[0].as_double().unwrap();
//     base.vel.y = motion_list[1].as_double().unwrap();
//     base.vel.z = motion_list[2].as_double().unwrap();

//     let rotation_list = root.get_list("Rotation").ok_or(invalid_tag("/Rotation not list"))?;
//     base.look.x = rotation_list[0].as_float().unwrap();
//     base.look.y = rotation_list[1].as_float().unwrap();

//     base.fall_distance = root.get_float("FallDistance").unwrap_or_default();
//     base.fire_ticks = root.get_short("Fire").unwrap_or_default().max(0) as u32;
//     base.air_ticks = root.get_short("Air").unwrap_or_default().max(0) as u32;
//     base.on_ground = root.get_boolean("OnGround").unwrap_or_default();

//     fn living_from_nbt<I>(base: &mut LivingEntity<I>, root: &NbtCompound) {

//         base.health = root.get_short("Health").unwrap_or(10).max(0) as u32;
//         // TODO: Hurt/Death/Attach time
        
//     }

//     fn projectile_from_nbt<I>(base: &mut ProjectileEntity<I>, root: &NbtCompound) {
        
//         let in_tile = root.get_byte("inTile").unwrap_or_default() as u8;
//         if in_tile != 0 {
//             base.kind.block_hit = Some((
//                 IVec3 {
//                     x: root.get_short("xTile").unwrap_or_default() as i32,  // WTF??
//                     y: root.get_short("yTile").unwrap_or_default() as i32,  // WTF??
//                     z: root.get_short("zTile").unwrap_or_default() as i32,  // WTF??
//                 },
//                 in_tile,
//                 root.get_byte("inData").unwrap_or_default() as u8,
//             ));
//         } else {
//             base.kind.block_hit = None;
//         }

//     }

//     match &mut *entity {
//         Entity::Arrow(base) => projectile_from_nbt(base, root),
//         Entity::Item(base) => {
//             base.health = root.get_short("Health").unwrap_or_default() as u8 as u32;
//             base.lifetime = root.get_short("Age").unwrap_or_default().max(0) as u32;
//             base.kind.stack = root.get_compound("Item").map(stack_from_nbt).unwrap_or_default();
//         }
//         Entity::Chicken(base) => living_from_nbt(base, root),
//         _ => ()
//     }

//     todo!()

// }


fn entity_kind_from_id(id: &str) -> Result<EntityKind, ChunkError> {
    Ok(match id {
        "Arrow" => EntityKind::Arrow,
        "Snowball" => EntityKind::Snowball,
        "Item" => EntityKind::Item,
        "Painting" => EntityKind::Painting,
        "Creeper" => EntityKind::Creeper,
        "Skeleton" => EntityKind::Skeleton,
        "Spider" => EntityKind::Spider,
        "Giant" => EntityKind::Giant,
        "Zombie" => EntityKind::Zombie,
        "Slime" => EntityKind::Slime,
        "Ghast" => EntityKind::Ghast,
        "PigZombie" => EntityKind::PigZombie,
        "Pig" => EntityKind::Pig,
        "Sheep" => EntityKind::Sheep,
        "Cow" => EntityKind::Cow,
        "Chicken" => EntityKind::Chicken,
        "Squid" => EntityKind::Squid,
        "Wolf" => EntityKind::Wolf,
        "PrimedTnt" => EntityKind::Tnt,
        "FallingSand" => EntityKind::FallingBlock,
        "Minecart" => EntityKind::Minecart,
        "Boat" => EntityKind::Boat,
        _ => return Err(ChunkError::InvalidEntityId(id.to_string())),
    })
}

#[inline]
fn invalid_tag(message: &str) -> ChunkError {
    ChunkError::InvalidTag(message.to_string())
}

/// Error type used together with `RegionResult` for every call on region file methods.
#[derive(thiserror::Error, Debug)]
pub enum ChunkError {
    #[error("{0}")]
    NbtError(#[from] NbtError),
    #[error("Not populated")]
    NotPopulated,
    #[error("Invalid tag: {0}")]
    InvalidTag(String),
    #[error("Invalid entity id: {0}")]
    InvalidEntityId(String),
    #[error("Invalid block entity id: {0}")]
    InvalidBlockEntityId(String),
}
