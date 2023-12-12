//! NBT serialization and deserialization for [`BlockEntity`] type.


use std::collections::HashMap;

use glam::IVec3;

use serde::de::{Deserializer, Visitor, SeqAccess};
use serde::ser::{Serializer, SerializeSeq};

use crate::block_entity::note_block::NoteBlockBlockEntity;
use crate::block_entity::dispenser::DispenserBlockEntity;
use crate::block_entity::furnace::FurnaceBlockEntity;
use crate::block_entity::jukebox::JukeboxBlockEntity;
use crate::block_entity::spawner::SpawnerBlockEntity;
use crate::block_entity::piston::PistonBlockEntity;
use crate::block_entity::chest::ChestBlockEntity;
use crate::block_entity::sign::SignBlockEntity;
use crate::block_entity::BlockEntity;
use crate::entity_new::EntityKind;
use crate::util::Face;

use super::slot_nbt::{SlotItemStackNbt, insert_slots, make_slots};
use super::entity_kind_nbt;


pub fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<HashMap<IVec3, Box<BlockEntity>>, D::Error> {

    struct SeqVisitor;
    impl<'de> Visitor<'de> for SeqVisitor {
        
        type Value = HashMap<IVec3, Box<BlockEntity>>;

        fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
            write!(formatter, "a sequence")
        }

        fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
        where
            A: SeqAccess<'de>, 
        {
            let mut block_entities = HashMap::with_capacity(seq.size_hint().unwrap_or(0));
            while let Some(nbt) = seq.next_element::<BlockEntityNbt>()? {
                let (pos, block_entity) = nbt.into_block_entity();
                block_entities.insert(pos, block_entity);
            }
            Ok(block_entities)
        }

    }

    deserializer.deserialize_seq(SeqVisitor)

}

pub fn serialize<S: Serializer>(value: &HashMap<IVec3, Box<BlockEntity>>, serializer: S) -> Result<S::Ok, S::Error> {

    let mut seq = serializer.serialize_seq(Some(value.len()))?;
    
    for (&pos, block_entity) in value {
        seq.serialize_element(&BlockEntityNbt::from_block_entity(pos, block_entity))?;
    }

    seq.end()

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
        slots: Vec<SlotItemStackNbt>,
    },
    Furnace {
        #[serde(rename = "Items")]
        slots: Vec<SlotItemStackNbt>,
    },
    #[serde(rename = "Trap")]
    Dispenser {
        #[serde(rename = "Items")]
        slots: Vec<SlotItemStackNbt>,
    },
    #[serde(rename = "MobSpawner")]
    Spawner {
        #[serde(rename = "EntityId", with = "entity_kind_nbt")]
        entity_kind: EntityKind,
        #[serde(rename = "Delay")]
        remaining_ticks: u16,
    },
    #[serde(rename = "Music")]
    NoteBlock {
        note: u8,
    },
    Piston {
        #[serde(rename = "blockId")]
        block: i32,
        #[serde(rename = "blockData")]
        metadata: i32,
        facing: i32,
        progress: f32,
        extending: bool,
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
    #[serde(rename = "RecordPlayer")]
    Jukebox {
        #[serde(rename = "Record")]
        record: u32
    },
}

impl BlockEntityNbt {
    
    /// Convert this raw block entity into the position and boxed block entity ready
    /// to be inserted into the chunk snapshot mapping.
    pub fn into_block_entity(self) -> (IVec3, Box<BlockEntity>) {

        let block_entity = Box::new(match self.kind {
            BlockEntityKindNbt::Chest { slots } => {
                let mut chest = ChestBlockEntity::default();
                insert_slots(slots, &mut chest.inv[..]);
                BlockEntity::Chest(chest)
            }
            BlockEntityKindNbt::Furnace { slots } => {
                let mut furnace = FurnaceBlockEntity::default();
                for slot in slots {
                    match slot.slot {
                        0 => furnace.input_stack = slot.stack,
                        1 => furnace.fuel_stack = slot.stack,
                        2 => furnace.output_stack = slot.stack,
                        _ => {}
                    }
                }
                BlockEntity::Furnace(furnace)
            }
            BlockEntityKindNbt::Dispenser { slots } => {
                let mut dispenser = DispenserBlockEntity::default();
                insert_slots(slots, &mut dispenser.inv[..]);
                BlockEntity::Dispenser(dispenser)
            }
            BlockEntityKindNbt::Spawner { entity_kind, remaining_ticks } => {
                let mut spawner = SpawnerBlockEntity::default();
                spawner.entity_kind = entity_kind;
                spawner.remaining_ticks = remaining_ticks as u32;
                BlockEntity::Spawner(spawner)
            }
            BlockEntityKindNbt::NoteBlock { note } => {
                let mut note_block = NoteBlockBlockEntity::default();
                note_block.note = note;
                BlockEntity::NoteBlock(note_block)
            }
            BlockEntityKindNbt::Piston { block, metadata, facing, progress, extending } => {
                let mut piston = PistonBlockEntity::default();
                piston.block = block as u8;
                piston.metadata = metadata as u8;
                piston.face = match facing {
                    0 => Face::NegY,
                    1 => Face::PosY,
                    2 => Face::NegZ,
                    3 => Face::PosZ,
                    4 => Face::NegX,
                    _ => Face::PosX,
                };
                piston.progress = progress;
                piston.extending = extending;
                BlockEntity::Piston(piston)
            }
            BlockEntityKindNbt::Sign { text1, text2, text3, text4 } => {
                let mut sign = SignBlockEntity::default();
                sign.lines[0] = text1;
                sign.lines[1] = text2;
                sign.lines[2] = text3;
                sign.lines[3] = text4;
                BlockEntity::Sign(sign)
            }
            BlockEntityKindNbt::Jukebox { record } => {
                BlockEntity::Jukebox(JukeboxBlockEntity { record })
            }
        });
    
        let pos = IVec3::new(self.x, self.y, self.z);
        (pos, block_entity)

    }

    pub fn from_block_entity(pos: IVec3, block_entity: &BlockEntity) -> Self {
        Self {
            x: pos.x,
            y: pos.y,
            z: pos.z,
            kind: match block_entity {
                BlockEntity::Chest(chest) => {
                    BlockEntityKindNbt::Chest { 
                        slots: make_slots(&chest.inv[..]),
                    }
                }
                BlockEntity::Furnace(furnace) => {
                    BlockEntityKindNbt::Furnace { 
                        slots: vec![
                            SlotItemStackNbt { slot: 0, stack: furnace.input_stack },
                            SlotItemStackNbt { slot: 1, stack: furnace.fuel_stack },
                            SlotItemStackNbt { slot: 2, stack: furnace.output_stack },
                        ]
                    }
                }
                BlockEntity::Dispenser(dispenser) => {
                    BlockEntityKindNbt::Chest { 
                        slots: make_slots(&dispenser.inv[..]),
                    }
                }
                BlockEntity::Spawner(spawner) => {
                    BlockEntityKindNbt::Spawner { 
                        entity_kind: spawner.entity_kind, 
                        remaining_ticks: spawner.remaining_ticks.min(u16::MAX as _) as u16,
                    }
                }
                BlockEntity::NoteBlock(note_block) => {
                    BlockEntityKindNbt::NoteBlock { 
                        note: note_block.note,
                    }
                }
                BlockEntity::Piston(piston) => {
                    BlockEntityKindNbt::Piston { 
                        block: piston.block as i32, 
                        metadata: piston.metadata as i32, 
                        facing: match piston.face {
                            Face::NegY => 0,
                            Face::PosY => 1,
                            Face::NegZ => 2,
                            Face::PosZ => 3,
                            Face::NegX => 4,
                            Face::PosX => 5,
                        }, 
                        progress: piston.progress, 
                        extending: piston.extending,
                    }
                }
                BlockEntity::Sign(sign) => {
                    BlockEntityKindNbt::Sign { 
                        text1: sign.lines[0].clone(), 
                        text2: sign.lines[1].clone(), 
                        text3: sign.lines[2].clone(), 
                        text4: sign.lines[3].clone(),
                    }
                }
                BlockEntity::Jukebox(jukebox) => {
                    BlockEntityKindNbt::Jukebox { 
                        record: jukebox.record,
                    }
                }
            },
        }
    }
    
}
