//! NBT serialization and deserialization for `Vec<Box<Entity>>` type.

use std::borrow::Cow;

use glam::IVec3;

use serde::de::{Deserializer, Visitor, SeqAccess};
use serde::ser::{Serializer, SerializeSeq};

use crate::entity::{self, Entity, PaintingOrientation, PaintingArt};
use crate::item::ItemStack;

use super::slot_nbt::SlotItemStackNbt;
use super::item_stack_nbt;


pub fn deserialize<'a, 'de, D: Deserializer<'de>>(deserializer: D) -> Result<Cow<'a, [Box<Entity>]>, D::Error> {

    /// Internal type to visit the sequence of entities.
    struct SeqVisitor;
    impl<'de> Visitor<'de> for SeqVisitor {

        type Value = Vec<Box<Entity>>;

        fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
            write!(formatter, "a sequence")
        }

        fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
        where
            A: SeqAccess<'de>, 
        {
            let mut entities = Vec::with_capacity(seq.size_hint().unwrap_or(0));
            while let Some(nbt) = seq.next_element::<EntityNbt>()? {
                entities.push(nbt.into_entity());
            }
            Ok(entities)
        }

    }

    deserializer.deserialize_seq(SeqVisitor).map(Cow::Owned)
    
}

pub fn serialize<'a, S: Serializer>(value: &Cow<'a, [Box<Entity>]>, serializer: S) -> Result<S::Ok, S::Error> {

    let mut seq = serializer.serialize_seq(Some(value.len()))?;

    for entity in &**value {
        seq.serialize_element(&EntityNbt::from_entity(&entity))?;
    }

    seq.end()

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
    kind: EntityKindNbt,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
#[serde(tag = "id")]
enum EntityKindNbt {
    Arrow {
        #[serde(flatten)]
        projectile: ProjectileEntityNbt,
        #[serde(rename = "player")]
        from_player: bool,
    },
    Snowball {
        #[serde(flatten)]
        projectile: ProjectileEntityNbt,
    },
    Item {
        #[serde(rename = "Health", default)]
        health: i16,
        #[serde(rename = "Age", default)]
        lifetime: u32,
        #[serde(with = "item_stack_nbt", flatten)]
        stack: ItemStack,
    },
    Painting {
        #[serde(rename = "Dir")]
        dir: i8,
        #[serde(rename = "Motive")]
        art: PaintingArt,
        #[serde(rename = "TileX")]
        x_block: i32,
        #[serde(rename = "TileY")]
        y_block: i32,
        #[serde(rename = "TileZ")]
        z_block: i32,
    },
    Creeper {
        #[serde(flatten)]
        living: LivingEntityNbt,
        powered: bool,
    },
    Skeleton {
        #[serde(flatten)]
        living: LivingEntityNbt,
    },
    Spider {
        #[serde(flatten)]
        living: LivingEntityNbt,
    },
    Giant {
        #[serde(flatten)]
        living: LivingEntityNbt,
    },
    Zombie {
        #[serde(flatten)]
        living: LivingEntityNbt,
    },
    Slime {
        #[serde(flatten)]
        living: LivingEntityNbt,
        #[serde(rename = "Size")]
        size: i32,
    },
    Ghast {
        #[serde(flatten)]
        living: LivingEntityNbt,
    },
    PigZombie {
        #[serde(flatten)]
        living: LivingEntityNbt,
        #[serde(rename = "Anger")]
        anger: i16,
    },
    Pig {
        #[serde(flatten)]
        living: LivingEntityNbt,
        #[serde(rename = "Saddle")]
        saddle: bool,
    },
    Sheep {
        #[serde(flatten)]
        living: LivingEntityNbt,
        #[serde(rename = "Sheared")]
        sheared: bool,
        #[serde(rename = "Color")]
        color: u8,
    },
    Cow {
        #[serde(flatten)]
        living: LivingEntityNbt,
    },
    Chicken {
        #[serde(flatten)]
        living: LivingEntityNbt,
    },
    Squid {
        #[serde(flatten)]
        living: LivingEntityNbt,
    },
    Wolf {
        #[serde(flatten)]
        living: LivingEntityNbt,
        #[serde(rename = "Angry", default)]
        angry: bool,
        #[serde(rename = "Sitting", default)]
        sitting: bool,
        #[serde(rename = "Owner", default)]
        owner: String,
    },
    #[serde(rename = "PrimedTnt")]
    Tnt {
        #[serde(rename = "Fuse")]
        fuse: i8,
    },
    #[serde(rename = "FallingSand")]
    FallingBlock {
        #[serde(rename = "Tile")]
        block: u8,
    },
    Minecart {
        #[serde(rename = "Type")]
        kind: i32,
        #[serde(flatten, default)]
        furnace: Option<MinecartFurnaceEntityNbt>,
        #[serde(flatten, default)]
        chest: Option<MinecartChestEntityNbt>,
    },
    Boat {},
}

/// The projectile NBT is really weird for MC, this means that projectiles cannot be 
/// saved beyond i16 range?
#[derive(Debug, serde::Serialize, serde::Deserialize)]
struct ProjectileEntityNbt {
    #[serde(rename = "xTile")]
    x_block: i16,
    #[serde(rename = "yTile")]
    y_block: i16,
    #[serde(rename = "zTile")]
    z_block: i16,
    #[serde(rename = "inTile")]
    in_block: u8,
    #[serde(rename = "inData")]
    in_metadata: u8,
    #[serde(rename = "inGround")]
    in_ground: bool,
    shake: i8,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
struct LivingEntityNbt {
    #[serde(rename = "Health", default = "living_entity_default_health")]
    health: i16,
    #[serde(rename = "HurtTime")]
    hurt_time: i16,
    #[serde(rename = "DeathTime")]
    death_time: i16,
    #[serde(rename = "AttackTime")]
    attack_time: i16,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
struct MinecartFurnaceEntityNbt {
    #[serde(rename = "PushX")]
    x_push: f64,
    #[serde(rename = "PushZ")]
    z_push: f64,
    fuel: i16,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
struct MinecartChestEntityNbt {
    #[serde(rename = "Items")]
    slots: Vec<SlotItemStackNbt>,
}

/// Default value for health of living entities.
#[inline(always)]
fn living_entity_default_health() -> i16 {
    10
}

impl EntityNbt {

    /// Transform this raw entity NBT into a boxed entity depending on the internal
    /// variant of the entity.
    fn into_entity(self) -> Box<Entity> {

        let mut entity = Box::new(match self.kind {
            EntityKindNbt::Arrow { projectile, from_player: _ } =>
                Entity::Arrow(projectile.into_entity()),
            EntityKindNbt::Snowball { projectile } =>
                Entity::Snowball(projectile.into_entity()),
            EntityKindNbt::Item { health, lifetime, stack } => {
                let mut item = entity::ItemEntity::default();
                item.health = health.max(0) as u32;
                item.lifetime = lifetime;
                item.kind.stack = stack;
                Entity::Item(item)
            }
            EntityKindNbt::Painting { dir, art, x_block, y_block, z_block } => {
                let mut painting = entity::PaintingEntity::default();
                painting.kind.art = art;
                painting.kind.orientation = match dir {
                    0 => PaintingOrientation::NegZ,
                    1 => PaintingOrientation::NegX,
                    2 => PaintingOrientation::PosZ,
                    _ => PaintingOrientation::PosX,
                };
                painting.kind.block_pos.x = x_block;
                painting.kind.block_pos.y = y_block;
                painting.kind.block_pos.z = z_block;
                Entity::Painting(painting)
            }
            EntityKindNbt::Creeper { living, powered } => {
                let mut creeper: entity::CreeperEntity = living.into_entity();
                creeper.kind.kind.powered = powered;
                Entity::Creeper(creeper)
            }
            EntityKindNbt::Skeleton { living } => 
                Entity::Skeleton(living.into_entity()),
            EntityKindNbt::Spider { living } => 
                Entity::Spider(living.into_entity()),
            EntityKindNbt::Giant { living } => 
                Entity::Giant(living.into_entity()),
            EntityKindNbt::Zombie { living } => 
                Entity::Zombie(living.into_entity()),
            EntityKindNbt::Slime { living, size } => {
                let mut slime: entity::SlimeEntity = living.into_entity();
                slime.kind.kind.size = size.clamp(0, 255) as u8;
                Entity::Slime(slime)
            }
            EntityKindNbt::Ghast { living } => 
                Entity::Ghast(living.into_entity()),
            EntityKindNbt::PigZombie { living, anger } => {
                let mut pig_zombie: entity::PigZombieEntity = living.into_entity();
                pig_zombie.kind.kind.anger = anger != 0;
                Entity::PigZombie(pig_zombie)
            }
            EntityKindNbt::Pig { living, saddle } => {
                let mut pig: entity::PigEntity = living.into_entity();
                pig.kind.kind.saddle = saddle;
                Entity::Pig(pig)
            }
            EntityKindNbt::Sheep { living, sheared, color } => {
                let mut sheep: entity::SheepEntity = living.into_entity();
                sheep.kind.kind.sheared = sheared;
                sheep.kind.kind.color = color;
                Entity::Sheep(sheep)
            }
            EntityKindNbt::Cow { living } =>
                Entity::Cow(living.into_entity()),
            EntityKindNbt::Chicken { living } =>
                Entity::Chicken(living.into_entity()),
            EntityKindNbt::Squid { living } => 
                Entity::Squid(living.into_entity()),
            EntityKindNbt::Wolf { living, angry, sitting, owner } => {
                let mut wolf: entity::WolfEntity = living.into_entity();
                wolf.kind.kind.angry = angry;
                wolf.kind.kind.sitting = sitting;
                wolf.kind.kind.owner = (!owner.is_empty()).then_some(owner);
                Entity::Wolf(wolf)
            }
            EntityKindNbt::Tnt { fuse } => {
                let mut tnt: entity::TntEntity = Default::default();
                tnt.kind.fuse_ticks = fuse.max(0) as u32;
                Entity::Tnt(tnt)
            }
            EntityKindNbt::FallingBlock { block } => {
                let mut falling_block: entity::FallingBlockEntity = Default::default();
                falling_block.kind.block_id = block;
                Entity::FallingBlock(falling_block)
            }
            EntityKindNbt::Minecart { kind, furnace, chest } => {
                let _ = (kind, furnace, chest);
                todo!()
            }
            EntityKindNbt::Boat {  } => 
                Entity::Boat(Default::default())
        });

        let base = entity.base_mut();
        // Position.
        base.pos.x = self.pos.0;
        base.pos.y = self.pos.1;
        base.pos.z = self.pos.2;
        // Velocity.
        base.vel.x = self.vel.0;
        base.vel.y = self.vel.1;
        base.vel.z = self.vel.2;
        // Look.
        base.look.x = self.look.0;
        base.look.y = self.look.1;
        // Misc.
        base.fall_distance = self.fall_distance;
        base.fire_ticks = self.fire_ticks.max(0) as u32;
        base.air_ticks = self.air_ticks.max(0) as u32;
        base.on_ground = self.on_ground;
        
        entity

    }

    fn from_entity(entity: &Entity) -> Option<Self> {

        let base = entity.base();

        if !base.persistent {
            return None;
        }

        Some(Self {
            pos: (base.pos.x, base.pos.y, base.pos.z),
            vel: (base.vel.x, base.vel.y, base.vel.z),
            look: (base.look.x, base.look.y),
            fall_distance: base.fall_distance,
            fire_ticks: base.fire_ticks.min(i16::MAX as _) as i16,
            air_ticks: base.air_ticks.min(i16::MAX as _) as i16,
            on_ground: base.on_ground,
            kind: match entity {
                Entity::Item(item) => {
                    EntityKindNbt::Item { 
                        health: item.health.min(i16::MAX as _) as i16, 
                        lifetime: item.lifetime, 
                        stack: item.kind.stack,
                    }
                }
                Entity::Painting(painting) => {
                    EntityKindNbt::Painting { 
                        dir: match painting.kind.orientation {
                            PaintingOrientation::NegZ => 0,
                            PaintingOrientation::NegX => 1,
                            PaintingOrientation::PosZ => 2,
                            PaintingOrientation::PosX => 3,
                        }, 
                        art: painting.kind.art, 
                        x_block: painting.kind.block_pos.x, 
                        y_block: painting.kind.block_pos.y, 
                        z_block: painting.kind.block_pos.z,
                    }
                }
                Entity::Boat(_) => {
                    EntityKindNbt::Boat { }
                }
                Entity::Minecart(_) => {
                    todo!()
                }
                Entity::FallingBlock(falling_block) => {
                    EntityKindNbt::FallingBlock { block: falling_block.kind.block_id }
                }
                Entity::Tnt(tnt) => {
                    EntityKindNbt::Tnt { fuse: tnt.kind.fuse_ticks.min(i8::MAX as _) as i8 }
                }
                Entity::Arrow(arrow) => {
                    EntityKindNbt::Arrow { 
                        projectile: ProjectileEntityNbt::from_entity(arrow), 
                        from_player: false,
                    }
                }
                Entity::Snowball(snowball) => {
                    EntityKindNbt::Snowball { projectile: ProjectileEntityNbt::from_entity(snowball) }
                }
                Entity::Ghast(ghast) => {
                    EntityKindNbt::Ghast { living: LivingEntityNbt::from_entity(ghast) }
                }
                Entity::Slime(slime) => {
                    EntityKindNbt::Slime { 
                        living: LivingEntityNbt::from_entity(slime),
                        size: slime.kind.kind.size as i32,
                    }
                }
                Entity::Pig(pig) => {
                    EntityKindNbt::Pig { 
                        living: LivingEntityNbt::from_entity(pig),
                        saddle: pig.kind.kind.saddle,
                    }
                }
                Entity::Chicken(chicken) => {
                    EntityKindNbt::Chicken { living: LivingEntityNbt::from_entity(chicken) }
                }
                Entity::Cow(cow) => {
                    EntityKindNbt::Cow { living: LivingEntityNbt::from_entity(cow) }
                }
                Entity::Sheep(sheep) => {
                    EntityKindNbt::Sheep { 
                        living: LivingEntityNbt::from_entity(sheep), 
                        sheared: sheep.kind.kind.sheared, 
                        color: sheep.kind.kind.color,
                    }
                }
                Entity::Squid(squid) => {
                    EntityKindNbt::Squid { living: LivingEntityNbt::from_entity(squid) }
                }
                Entity::Wolf(wolf) => {
                    EntityKindNbt::Wolf { 
                        living: LivingEntityNbt::from_entity(wolf), 
                        angry: wolf.kind.kind.angry, 
                        sitting: wolf.kind.kind.sitting, 
                        owner: wolf.kind.kind.owner.clone().unwrap_or_default(),
                    }
                }
                Entity::Creeper(creeper) => {
                    EntityKindNbt::Creeper { 
                        living: LivingEntityNbt::from_entity(creeper), 
                        powered: creeper.kind.kind.powered,
                    }
                }
                Entity::Giant(giant) => {
                    EntityKindNbt::Giant { living: LivingEntityNbt::from_entity(giant) }
                }
                Entity::PigZombie(pig_zombie) => {
                    EntityKindNbt::PigZombie { 
                        living: LivingEntityNbt::from_entity(pig_zombie),
                        anger: pig_zombie.kind.kind.anger as i16,
                    }
                }
                Entity::Skeleton(skeleton) => {
                    EntityKindNbt::Skeleton { living: LivingEntityNbt::from_entity(skeleton) }
                }
                Entity::Spider(spider) => {
                    EntityKindNbt::Spider { living: LivingEntityNbt::from_entity(spider) }
                }
                Entity::Zombie(zombie) => {
                    EntityKindNbt::Zombie { living: LivingEntityNbt::from_entity(zombie) }
                }
                // Other entities could not be serialized in NBT.
                _ => return None,
            },
        })

    }

}

impl ProjectileEntityNbt {

    /// Apply this projectile entity NBT to a projectile entity data.
    #[inline]
    fn into_entity<I>(self) -> entity::ProjectileEntity<I>
    where
        entity::ProjectileEntity<I>: Default,
    {
        
        let mut data = entity::ProjectileEntity::<I>::default();

        if self.in_ground {
            data.kind.block_hit = Some((
                IVec3::new(self.x_block as i32, self.y_block as i32, self.z_block as i32),
                self.in_block,
                self.in_metadata,
            ));
        } else {
            data.kind.block_hit = None;
        }

        data.kind.shake = self.shake.max(0) as u8;
        data

    }

    fn from_entity<I>(entity: &entity::ProjectileEntity<I>) -> Self {
        let (pos, block, metadata) = entity.kind.block_hit.unwrap_or_default();
        Self {
            x_block: pos.x as i16,
            y_block: pos.y as i16,
            z_block: pos.z as i16,
            in_block: block,
            in_metadata: metadata,
            in_ground: entity.kind.block_hit.is_some(),
            shake: entity.kind.shake.min(i8::MAX as _) as i8,
        }
    }

}

impl LivingEntityNbt {

    /// Apply this living entity NBT to a living entity data.
    #[inline]
    fn into_entity<I>(self) -> entity::LivingEntity<I>
    where
        entity::LivingEntity<I>: Default,
    {
        let mut data = entity::LivingEntity::<I>::default();
        data.health = self.health.max(0) as u32;
        data
    }

    fn from_entity<I>(entity: &entity::LivingEntity<I>) -> Self {
        Self {
            health: entity.health.min(i16::MAX as _) as i16,
            hurt_time: 0,
            death_time: 0,
            attack_time: 0,
        }
    }

}
