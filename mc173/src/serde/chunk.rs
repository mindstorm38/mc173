//! Chunk serialization and deserialization from NBT compound.

use std::sync::Arc;

use glam::IVec3;

use crate::entity::{Entity, EntityKind, ProjectileEntity, LivingEntity};
use crate::item::ItemStack;
use crate::world::ChunkSnapshot;

use super::nbt::{Nbt, NbtError, NbtCompound};


/// Read a chunk and all of its components from the given NBT compound.
pub fn from_nbt(root: &Nbt, only_populated: bool) -> Result<ChunkSnapshot, ChunkError> {

    let root = root.as_compound().ok_or(invalid_tag("/ not compound"))?;
    let level = root.get_compound("Level").ok_or(invalid_tag("/Level not compound"))?;

    // Directly abort if the chunk is not populated yet.
    if only_populated && !level.get_boolean("TerrainPopulated").unwrap_or(true) {
        return Err(ChunkError::NotPopulated);
    }

    let cx = level.get_int("xPos").ok_or(invalid_tag("/Level/xPos not int"))?;
    let cz = level.get_int("zPos").ok_or(invalid_tag("/Level/zPos not int"))?;

    let mut snapshot = ChunkSnapshot::new(cx, cz);
    let chunk = Arc::get_mut(&mut snapshot.chunk).unwrap();

    let block = level.get_byte_array("Blocks").ok_or(invalid_tag("/Level/Blocks not byte array"))?;
    chunk.block.copy_from_slice(block);
    let metadata = level.get_byte_array("Data").ok_or(invalid_tag("/Level/Data not byte array"))?;
    chunk.metadata.inner.copy_from_slice(metadata);
    let block_light = level.get_byte_array("BlockLight").ok_or(invalid_tag("/Level/BlockLight not byte array"))?;
    chunk.block_light.inner.copy_from_slice(block_light);
    let sky_light = level.get_byte_array("SkyLight").ok_or(invalid_tag("/Level/SkyLight not byte array"))?;
    chunk.sky_light.inner.copy_from_slice(sky_light);
    let height_map = level.get_byte_array("HeightMap").ok_or(invalid_tag("/Level/HeightMap not byte array"))?;
    chunk.height.copy_from_slice(height_map);

    let entities = level.get_list("Entities").ok_or(invalid_tag("/Level/Entities not list"))?;
    for entity in entities {
        let entity = entity.as_compound().ok_or(invalid_tag("/Level/Entities/[] not compound"))?;
    }

    let block_entities = level.get_list("TileEntities").ok_or(invalid_tag("/Level/TileEntities not list"))?;
    for block_entity in block_entities {
        let block_entity = block_entity.as_compound().ok_or(invalid_tag("/Level/TileEntities/[] not compound"))?;
    }

    Ok(snapshot)

}

pub fn entity_from_nbt(root: &Nbt) -> Result<Box<Entity>, ChunkError> {

    let root = root.as_compound().ok_or(invalid_tag("/ not compound"))?;
    let entity_id = root.get_string("id").ok_or(invalid_tag("/id not string"))?;

    let mut entity = match entity_id {
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
        _ => return Err(ChunkError::InvalidEntityId(entity_id.to_string())),
    }.new_default();

    let base = entity.base_mut();

    let pos_list = root.get_list("Pos").ok_or(invalid_tag("/Pos not list"))?;
    base.pos.x = pos_list[0].as_double().unwrap();
    base.pos.y = pos_list[1].as_double().unwrap();
    base.pos.z = pos_list[2].as_double().unwrap();

    let motion_list = root.get_list("Motion").ok_or(invalid_tag("/Motion not list"))?;
    base.vel.x = motion_list[0].as_double().unwrap();
    base.vel.y = motion_list[1].as_double().unwrap();
    base.vel.z = motion_list[2].as_double().unwrap();

    let rotation_list = root.get_list("Rotation").ok_or(invalid_tag("/Rotation not list"))?;
    base.look.x = rotation_list[0].as_float().unwrap();
    base.look.y = rotation_list[1].as_float().unwrap();

    base.fall_distance = root.get_float("FallDistance").unwrap_or_default();
    base.fire_ticks = root.get_short("Fire").unwrap_or_default().max(0) as u32;
    base.air_ticks = root.get_short("Air").unwrap_or_default().max(0) as u32;
    base.on_ground = root.get_boolean("OnGround").unwrap_or_default();

    fn living_from_nbt<I>(base: &mut LivingEntity<I>, root: &NbtCompound) {

        base.health = root.get_short("Health").unwrap_or(10).max(0) as u32;
        // TODO: Hurt/Death/Attach time
        
    }

    fn projectile_from_nbt<I>(base: &mut ProjectileEntity<I>, root: &NbtCompound) {
        
        let in_tile = root.get_byte("inTile").unwrap_or_default() as u8;
        if in_tile != 0 {
            base.kind.block_hit = Some((
                IVec3 {
                    x: root.get_short("xTile").unwrap_or_default() as i32,  // WTF??
                    y: root.get_short("yTile").unwrap_or_default() as i32,  // WTF??
                    z: root.get_short("zTile").unwrap_or_default() as i32,  // WTF??
                },
                in_tile,
                root.get_byte("inData").unwrap_or_default() as u8,
            ));
        } else {
            base.kind.block_hit = None;
        }

    }

    match &mut *entity {
        Entity::Arrow(base) => projectile_from_nbt(base, root),
        Entity::Item(base) => {
            base.health = root.get_short("Health").unwrap_or_default() as u8 as u32;
            base.lifetime = root.get_short("Age").unwrap_or_default().max(0) as u32;
            base.kind.stack = root.get_compound("Item").map(stack_from_nbt).unwrap_or_default();
        }
        Entity::Chicken(base) => living_from_nbt(base, root),
        _ => ()
    }

    todo!()

}

/// Read an item stack from given nbt compound.
fn stack_from_nbt(root: &NbtCompound) -> ItemStack {
    ItemStack { 
        id: root.get_short("id").unwrap_or_default().max(0) as u16, 
        size: root.get_byte("Count").unwrap_or_default().max(0) as u16, 
        damage: root.get_short("Damage").unwrap_or_default().max(0) as u16,
    }
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
}
