//! NBT serialization and deserialization for `Vec<Box<Entity>>` type.

use std::cell::RefCell;
use std::borrow::Cow;

use glam::IVec3;

use serde::de::{Deserializer, Visitor, SeqAccess};
use serde::ser::Serializer;

use crate::entity::{self as e, 
    Entity, 
    Base, Projectile, Living,
    PaintingOrientation, PaintingArt};

use crate::item::ItemStack;

use super::slot_nbt::{SlotItemStackNbt, insert_slots, make_slots};
use super::item_stack_nbt;


// Various thread local vectors that are used to avoid frequent reallocation of 
// temporary vector used in the logic code.
thread_local! {
    /// This thread local vector is used to temporally store entities or block entities 
    /// indices that should be removed just after the update loop.
    static SERIALIZE_ENTITIES: RefCell<Vec<EntityNbt>> = const { RefCell::new(Vec::new()) };
}


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
    SERIALIZE_ENTITIES.with_borrow_mut(move |entities_nbt| {
        entities_nbt.extend(value.iter().filter_map(|entity| EntityNbt::from_entity(&entity)));
        serializer.collect_seq(entities_nbt.drain(..))
    })
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
    fire_time: i16,
    #[serde(rename = "Air", default)]
    air_time: i16,
    #[serde(rename = "OnGround", default)]
    on_ground: bool,
    #[serde(flatten)]
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
        #[serde(rename = "Item", with = "item_stack_nbt")]
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
        chest: Option<MinecartChestEntityNbt>,
        #[serde(flatten, default)]
        furnace: Option<MinecartFurnaceEntityNbt>,
    },
    Boat {},
}

/// The projectile NBT is really weird for MC, this means that projectiles cannot be 
/// saved beyond i16 range?
#[derive(Debug, serde::Serialize, serde::Deserialize)]
struct ProjectileEntityNbt {
    #[serde(rename = "xTile")]
    block_x: i16,
    #[serde(rename = "yTile")]
    block_y: i16,
    #[serde(rename = "zTile")]
    block_z: i16,
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
struct MinecartChestEntityNbt {
    #[serde(rename = "Items")]
    slots: Vec<SlotItemStackNbt>,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
struct MinecartFurnaceEntityNbt {
    #[serde(rename = "PushX")]
    push_x: f64,
    #[serde(rename = "PushZ")]
    push_z: f64,
    fuel: i16,
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

        let mut entity = match self.kind {
            EntityKindNbt::Arrow { projectile, from_player: _ } =>
                e::Arrow::new_with(|b, p, _| projectile.apply_entity(b, p)),
            EntityKindNbt::Snowball { projectile } =>
                e::Snowball::new_with(|b, p, _| projectile.apply_entity(b, p)),
            EntityKindNbt::Item { health, lifetime, stack } =>
                e::Item::new_with(|b, item| {
                    b.health = health.max(0) as u32;
                    b.lifetime = lifetime;
                    item.stack = stack;
                }),
            EntityKindNbt::Painting { dir, art, x_block, y_block, z_block } =>
                e::Painting::new_with(|_, painting| {
                    painting.art = art;
                    painting.orientation = match dir {
                        0 => PaintingOrientation::NegZ,
                        1 => PaintingOrientation::NegX,
                        2 => PaintingOrientation::PosZ,
                        _ => PaintingOrientation::PosX,
                    };
                    painting.block_pos.x = x_block;
                    painting.block_pos.y = y_block;
                    painting.block_pos.z = z_block;
                }),
            EntityKindNbt::Creeper { living, powered } =>
                e::Creeper::new_with(|b, l, creeper| {
                    living.apply_entity(b, l);
                    creeper.powered = powered;
                }),
            EntityKindNbt::Skeleton { living } => 
                e::Skeleton::new_with(|b, l, _| living.apply_entity(b, l)),
            EntityKindNbt::Spider { living } => 
                e::Spider::new_with(|b, l, _| living.apply_entity(b, l)),
            EntityKindNbt::Giant { living } => 
                e::Giant::new_with(|b, l, _| living.apply_entity(b, l)),
            EntityKindNbt::Zombie { living } => 
                e::Zombie::new_with(|b, l, _| living.apply_entity(b, l)),
            EntityKindNbt::Slime { living, size } =>
                e::Slime::new_with(|b, l, slime| {
                    living.apply_entity(b, l);
                    slime.size = size.clamp(0, 255) as u8;
                }),
            EntityKindNbt::Ghast { living } => 
                e::Ghast::new_with(|b, l, _| living.apply_entity(b, l)),
            EntityKindNbt::PigZombie { living, anger } => 
                e::PigZombie::new_with(|b, l, pig_zombie| {
                    living.apply_entity(b, l);
                    pig_zombie.anger = anger != 0;
                }),
            EntityKindNbt::Pig { living, saddle } =>
                e::Pig::new_with(|b, l, pig| {
                    living.apply_entity(b, l);
                    pig.saddle = saddle;
                }),
            EntityKindNbt::Sheep { living, sheared, color } =>
                e::Sheep::new_with(|b, l, sheep| {
                    living.apply_entity(b, l);
                    sheep.sheared = sheared;
                    sheep.color = color;
                }),
            EntityKindNbt::Cow { living } =>
                e::Cow::new_with(|b, l, _| living.apply_entity(b, l)),
            EntityKindNbt::Chicken { living } =>
                e::Chicken::new_with(|b, l, _| living.apply_entity(b, l)),
            EntityKindNbt::Squid { living } => 
                e::Squid::new_with(|b, l, _| living.apply_entity(b, l)),
            EntityKindNbt::Wolf { living, angry, sitting, owner } => 
                e::Wolf::new_with(|b, l, wolf| {
                    living.apply_entity(b, l);
                    wolf.angry = angry;
                    wolf.sitting = sitting;
                    wolf.owner = (!owner.is_empty()).then_some(owner);
                }),
            EntityKindNbt::Tnt { fuse } =>
                e::Tnt::new_with(|_, tnt| {
                    tnt.fuse_ticks = fuse.max(0) as u32;
                }),
            EntityKindNbt::FallingBlock { block } =>
                e::FallingBlock::new_with(|_, falling_block| {
                    falling_block.block_id = block;
                }),
            EntityKindNbt::Minecart { kind, chest, furnace } =>
                e::Minecart::new_with(|_, minecart| {

                    match kind {
                        1 => { // Chest minecart
                            if let Some(chest) = chest {
                                let mut inv: Box<[ItemStack; 27]> = Box::default();
                                insert_slots(chest.slots, &mut inv[..]);
                                *minecart = e::Minecart::Chest { inv };
                            }
                        }
                        2 => { // Furnace minecart
                            if let Some(furnace) = furnace {
                                *minecart = e::Minecart::Furnace { 
                                    fuel: furnace.fuel.max(0) as u32,
                                    push_x: furnace.push_x,
                                    push_z: furnace.push_z,
                                }
                            }
                        }
                        _ => {} // Normal minecart, no need to change default.
                    }

                }),
            EntityKindNbt::Boat {  } => 
                e::Boat::new_with(|_, _| {}),
        };

        let base = &mut entity.0;
        // Deserialized entities are persistent by definition.
        base.persistent = true;
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
        base.fire_time = self.fire_time.max(0) as u32;
        base.air_time = self.air_time.max(0) as u32;
        base.on_ground = self.on_ground;
        
        entity

    }

    fn from_entity(entity: &Entity) -> Option<Self> {

        let Entity(base, base_kind) = entity;

        if !base.persistent {
            return None;
        }

        Some(Self {
            pos: (base.pos.x, base.pos.y, base.pos.z),
            vel: (base.vel.x, base.vel.y, base.vel.z),
            look: (base.look.x, base.look.y),
            fall_distance: base.fall_distance,
            fire_time: base.fire_time.min(i16::MAX as _) as i16,
            air_time: base.air_time.min(i16::MAX as _) as i16,
            on_ground: base.on_ground,
            kind: match base_kind {
                e::BaseKind::Item(item) =>
                    EntityKindNbt::Item { 
                        health: base.health.min(i16::MAX as _) as i16, 
                        lifetime: base.lifetime, 
                        stack: item.stack,
                    },
                e::BaseKind::Painting(painting) =>
                    EntityKindNbt::Painting { 
                        dir: match painting.orientation {
                            PaintingOrientation::NegZ => 0,
                            PaintingOrientation::NegX => 1,
                            PaintingOrientation::PosZ => 2,
                            PaintingOrientation::PosX => 3,
                        }, 
                        art: painting.art, 
                        x_block: painting.block_pos.x, 
                        y_block: painting.block_pos.y, 
                        z_block: painting.block_pos.z,
                    },
                e::BaseKind::Boat(_boat) =>
                    EntityKindNbt::Boat { },
                e::BaseKind::Minecart(e::Minecart::Normal) =>
                    EntityKindNbt::Minecart { kind: 0, chest: None, furnace: None },
                e::BaseKind::Minecart(e::Minecart::Chest { inv }) =>
                    EntityKindNbt::Minecart { 
                        kind: 1, 
                        chest: Some(MinecartChestEntityNbt { 
                            slots: make_slots(&inv[..]),
                        }), 
                        furnace: None,
                    },
                &e::BaseKind::Minecart(e::Minecart::Furnace { fuel, push_x, push_z }) =>
                    EntityKindNbt::Minecart { 
                        kind: 2, 
                        chest: None, 
                        furnace: Some(MinecartFurnaceEntityNbt { 
                            push_x, 
                            push_z, 
                            fuel: fuel.min(i16::MAX as _) as i16,
                        }),
                    },
                e::BaseKind::FallingBlock(falling_block) => 
                    EntityKindNbt::FallingBlock { 
                        block: falling_block.block_id 
                    },
                e::BaseKind::Tnt(tnt) => 
                    EntityKindNbt::Tnt { 
                        fuse: tnt.fuse_ticks.min(i8::MAX as _) as i8 
                    },
                e::BaseKind::Projectile(projectile, projectile_kind) => {

                    let projectile = ProjectileEntityNbt::from_entity(base, projectile);

                    match projectile_kind {
                        e::ProjectileKind::Arrow(_) => 
                            EntityKindNbt::Arrow { 
                                projectile, 
                                from_player: false,
                            },
                        e::ProjectileKind::Snowball(_) => 
                            EntityKindNbt::Snowball { projectile },
                        // Other projectile entities cannot be serialized.
                        _ => return None,
                    }

                }
                e::BaseKind::Living(living, living_kind) => {

                    let living = LivingEntityNbt::from_entity(base, living);

                    match living_kind {
                        e::LivingKind::Ghast(_) => 
                            EntityKindNbt::Ghast { living },
                        e::LivingKind::Slime(slime) => 
                            EntityKindNbt::Slime { 
                                living,
                                size: slime.size as i32,
                            },
                        e::LivingKind::Pig(pig) => 
                            EntityKindNbt::Pig { 
                                living,
                                saddle: pig.saddle,
                            },
                        e::LivingKind::Chicken(_) =>
                            EntityKindNbt::Chicken { living },
                        e::LivingKind::Cow(_) => 
                            EntityKindNbt::Cow { living },
                        e::LivingKind::Sheep(sheep) =>
                            EntityKindNbt::Sheep { 
                                living,
                                sheared: sheep.sheared, 
                                color: sheep.color,
                            },
                        e::LivingKind::Squid(_) => 
                            EntityKindNbt::Squid { living },
                        e::LivingKind::Wolf(wolf) => 
                            EntityKindNbt::Wolf { 
                                living, 
                                angry: wolf.angry, 
                                sitting: wolf.sitting, 
                                owner: wolf.owner.clone().unwrap_or_default(),
                            },
                        e::LivingKind::Creeper(creeper) => 
                            EntityKindNbt::Creeper { 
                                living, 
                                powered: creeper.powered,
                            },
                        e::LivingKind::Giant(_) => 
                            EntityKindNbt::Giant { living },
                        e::LivingKind::PigZombie(pig_zombie) => 
                            EntityKindNbt::PigZombie { 
                                living,
                                anger: pig_zombie.anger as i16,
                            },
                        e::LivingKind::Skeleton(_) => 
                            EntityKindNbt::Skeleton { living },
                        e::LivingKind::Spider(_) => 
                            EntityKindNbt::Spider { living },
                        e::LivingKind::Zombie(_) => 
                            EntityKindNbt::Zombie { living },
                        // Other living entities cannot be serialized.
                        _ => return None,
                    }

                }
                // Other entities could not be serialized in NBT.
                _ => return None,
            }
        })

    }

}

impl ProjectileEntityNbt {

    /// Apply this projectile entity NBT to the given data structures.
    fn apply_entity(self, _base: &mut Base, projectile: &mut Projectile) {
        
        if self.in_ground {
            projectile.block_hit = Some((
                IVec3::new(self.block_x as i32, self.block_y as i32, self.block_z as i32),
                self.in_block,
                self.in_metadata,
            ));
        } else {
            projectile.block_hit = None;
        }

        projectile.shake = self.shake.max(0) as u8;

    }

    fn from_entity(_base: &Base, projectile: &Projectile) -> Self {
        let (pos, block, metadata) = projectile.block_hit.unwrap_or_default();
        Self {
            block_x: pos.x as i16,
            block_y: pos.y as i16,
            block_z: pos.z as i16,
            in_block: block,
            in_metadata: metadata,
            in_ground: projectile.block_hit.is_some(),
            shake: projectile.shake.min(i8::MAX as _) as i8,
        }
    }

}

impl LivingEntityNbt {

    /// Apply this projectile entity NBT to the given data structures.
    fn apply_entity(self, base: &mut Base, living: &mut Living) {
        base.health = self.health.max(0) as u32;
        living.hurt_time = self.hurt_time.max(0) as u16;
        living.death_time = self.death_time.max(0) as u16;
        living.attack_time = self.attack_time.max(0) as u16;
    }

    fn from_entity(base: &Base, living: &Living) -> Self {
        Self {
            health: base.health.min(i16::MAX as _) as i16,
            hurt_time: living.hurt_time.min(i16::MAX as _) as i16,
            death_time: living.death_time.min(i16::MAX as _) as i16,
            attack_time: living.attack_time.min(i16::MAX as _) as i16,
        }
    }

}
