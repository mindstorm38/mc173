//! Chunk serialization and deserialization from NBT compound.

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
        snapshot.entities.push(entity_from_nbt(entity)?);
    }

    let block_entities = level.get_list("TileEntities").ok_or(invalid_tag("/Level/TileEntities not list"))?;
    for block_entity in block_entities {
        let (pos, block_entity) = block_entity_from_nbt(block_entity)?;
        snapshot.block_entities.insert(pos, block_entity);
    }

    Ok(snapshot)

}

/// Decode an entity from NBT.
pub fn entity_from_nbt(root: &Nbt) -> Result<Box<Entity>, ChunkError> {

    let root = root.as_compound().ok_or(invalid_tag("/ not compound"))?;
    let entity_id = root.get_string("id").ok_or(invalid_tag("/id not string"))?;

    let mut entity = entity_kind_from_id(entity_id)?.new_default();

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

/// Decode an block entity from NBT.
pub fn block_entity_from_nbt(root: &Nbt) -> Result<(IVec3, Box<BlockEntity>), ChunkError> {

    let root = root.as_compound().ok_or(invalid_tag("/ not compound"))?;
    let entity_id = root.get_string("id").ok_or(invalid_tag("/id not string"))?;

    let pos = IVec3 {
        x: root.get_int("x").ok_or(invalid_tag("/x not int"))?,
        y: root.get_int("y").ok_or(invalid_tag("/y not int"))?,
        z: root.get_int("z").ok_or(invalid_tag("/z not int"))?,
    };

    /// Internal function to iterate over block entity's inventory slots.
    fn iter_slots_from_nbt(root: &NbtCompound) -> Result<impl Iterator<Item = (usize, ItemStack)> + '_, ChunkError> {
        let items = root.get_list("Items").ok_or(invalid_tag("/Items not list"))?;
        let iter = items.iter()
            .filter_map(|item| item.as_compound())
            .map(|item| {
                let slot = item.get_byte("Slot").unwrap_or(0) as u8 as usize;
                (slot, stack_from_nbt(item))
            });
        Ok(iter)
    }

    let block_entity = Box::new(match entity_id {
        "Chest" => {
            let mut chest = ChestBlockEntity::default();
            for (slot, stack) in iter_slots_from_nbt(root)? {
                if slot < chest.inv.len() {
                    chest.inv[slot] = stack;
                }
            }
            BlockEntity::Chest(chest)
        }
        "Furnace" => {
            let mut furnace = FurnaceBlockEntity::default();
            for (slot, stack) in iter_slots_from_nbt(root)? {
                match slot {
                    0 => furnace.input_stack = stack,
                    1 => furnace.fuel_stack = stack,
                    2 => furnace.output_stack = stack,
                    _ => {}
                }
            }
            BlockEntity::Furnace(furnace)
        }
        "RecordPlayer" => {
            let mut jukebox = JukeboxBlockEntity::default();
            jukebox.record = root.get_int("Record").unwrap_or(0).max(0) as u32;
            BlockEntity::Jukebox(jukebox)
        }
        "Trap" => {
            let mut dispenser = DispenserBlockEntity::default();
            for (slot, stack) in iter_slots_from_nbt(root)? {
                if slot < dispenser.inv.len() {
                    dispenser.inv[slot] = stack;
                }
            }
            BlockEntity::Dispenser(dispenser)
        }
        "Sign" => {
            let mut sign = SignBlockEntity::default();
            for (i, key) in ["Text1", "Text2", "Text3", "Text4"].into_iter().enumerate() {
                sign.lines[i] = root.get_string(key).unwrap_or_default().to_string();
            }
            BlockEntity::Sign(sign)
        }
        "MobSpawner" => {
            let mut spawner = SpawnerBlockEntity::default();
            spawner.entity_kind = entity_kind_from_id(root.get_string("EntityId").unwrap_or_default())?;
            spawner.remaining_ticks = root.get_short("Delay").unwrap_or(0).max(0) as u32;
            BlockEntity::Spawner(spawner)
        }
        "Music" => {
            let mut note_block = NoteBlockBlockEntity::default();
            note_block.note = root.get_byte("note").unwrap_or(0).clamp(0, 24) as u8;
            BlockEntity::NoteBlock(note_block)
        }
        "Piston" => {
            let mut piston = PistonBlockEntity::default();
            piston.id = root.get_int("blockId").unwrap_or(0).clamp(0, 255) as u8;
            piston.metadata = root.get_int("blockData").unwrap_or(0).clamp(0, 255) as u8;
            piston.face = match root.get_int("facing").unwrap_or(0) {
                0 => Face::NegY,
                1 => Face::PosY,
                2 => Face::NegZ,
                3 => Face::PosZ,
                4 => Face::NegX,
                _ => Face::PosX,
            };
            piston.progress = root.get_float("progress").unwrap_or(0.0);
            piston.extending = root.get_boolean("extending").unwrap_or(false);
            BlockEntity::Piston(piston)
        }
        _ => return Err(ChunkError::InvalidBlockEntityId(entity_id.to_string())),
    });

    Ok((pos, block_entity))

}

/// Read an item stack from given nbt compound.
fn stack_from_nbt(root: &NbtCompound) -> ItemStack {
    ItemStack { 
        id: root.get_short("id").unwrap_or_default().max(0) as u16, 
        size: root.get_byte("Count").unwrap_or_default().max(0) as u16, 
        damage: root.get_short("Damage").unwrap_or_default().max(0) as u16,
    }
}

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
