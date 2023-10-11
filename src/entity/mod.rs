//! Entity data structures, no logic is defined here.

use glam::{DVec3, IVec3, Vec2};

use crate::rand::JavaRandom;
use crate::world::World;


/// Base trait for implementing entity behaviors.
pub trait Entity {

    /// Tick this entity and update its internal components.
    fn tick(&mut self, world: &World);

    /// Return the base entity component.
    fn base(&self) -> &BaseEntity;

}


/// Base class for entity.
#[derive(Debug, Default)]
pub struct BaseEntity {
    /// The internal entity id.
    pub id: u32,
    /// The current entity position.
    pub pos: DVec3,
    /// The current entity velocity.
    pub vel: DVec3,
    /// Yaw/Pitch look.
    pub look: Vec2,
    /// If this entity is ridden, this contains its entity id.
    pub rider_id: Option<u32>,
    /// Lifetime of the entity, in ticks.
    pub lifetime: u32,
    /// The health.
    pub health: u32,
    /// The random number generator used for this entity.
    pub random: JavaRandom,
    /// Specialized kind of entity.
    pub kind: EntityKind,
}

#[derive(Debug, Default)]
pub enum EntityKind {
    #[default]
    None,
    Projectile(ProjectileEntity),
    Item(ItemEntity),
    Painting(PaintingEntity),
    Living(LivingEntity),
}

#[derive(Debug)]
pub struct ProjectileEntity {
    pub tile_pos: Option<IVec3>,
    pub ticks_in_ground: Option<u32>,
    pub ticks_in_air: u32,
    pub shaking: bool,
    pub kind: ProjectileKind,
}

#[derive(Debug)]
pub enum ProjectileKind {
    Arrow,
    Snowball,
}

#[derive(Debug)]
pub struct ItemEntity {
    pub age: u32,
    pub delay_before_pickup: u32,
}

#[derive(Debug)]
pub struct PaintingEntity {
    pub pos: IVec3,
    pub orientation: PaintingOrientation,
    pub art: PaintingArt,
}

#[derive(Debug)]
pub enum PaintingOrientation {
    North,
    East,
    South,
    West,
}

#[derive(Debug)]
pub enum PaintingArt {
    Kebab,
    Aztec,
    Alban,
    Aztec2,
    Bomb,
    Plant,
    Wasteland,
    Pool,
    Courbet,
    Sea,
    Sunset,
    Creebet,
    Wanderer,
    Graham,
    Match,
    Bust,
    Stage,
    Void,
    SkullAndRoses,
    Fighters,
    Pointer,
    Pigscene,
    BurningSkull,
    Skeleton,
    DonkeyKong,
}

#[derive(Debug)]
pub struct LivingEntity {
    pub kind: LivingKind,
}

#[derive(Debug)]
pub enum LivingKind {
    Player(PlayerEntity),
    Mob,
    Slime,
}

#[derive(Debug)]
pub struct PlayerEntity {
    pub username: String,
}
