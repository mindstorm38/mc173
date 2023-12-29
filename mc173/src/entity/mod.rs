//! Trying a new architecture for entity structure (again!!!).

use glam::{DVec3, Vec2, IVec3};

use tracing::instrument;

use crate::util::{BoundingBox, JavaRandom};
use crate::item::ItemStack;
use crate::world::World;

pub mod common;

mod tick;
mod tick_state;
mod tick_ai;
mod tick_attack;

use tick_state::tick_state;
use tick_ai::tick_ai;
use tick_attack::tick_attack;


/// Kind of entity, without actual data. This enumeration can be used to construct a
/// real entity instance with default values, to be modified later.
#[derive(Debug, Clone, Copy)]
pub enum EntityKind {
    Item,
    Painting,
    Boat,
    Minecart,
    Fish,
    LightningBolt,
    FallingBlock,
    Tnt,
    Arrow,
    Egg,
    Fireball,
    Snowball,
    Player,
    Ghast,
    Slime,
    Pig,
    Chicken,
    Cow,
    Sheep,
    Squid,
    Wolf,
    Creeper,
    Giant,
    PigZombie,
    Skeleton,
    Spider,
    Zombie,
}

/// Base type that contains all entity types, this is composed of the entity base data,
/// which is common to all entities, and the base kind that is the first sub division in
/// entities. Each subdivision in the entity family tree is composed of the family's
/// common data as the first tuple element, and the kind of entity as the second element.
#[derive(Debug, Clone)]
pub struct Entity(pub Base, pub BaseKind);

/// Kind of base entity.
#[derive(Debug, Clone)]
pub enum BaseKind {
    Item(Item),
    Painting(Painting),
    Boat(Boat),
    Minecart(Minecart),
    Fish(Fish),
    LightningBolt(LightningBolt),
    FallingBlock(FallingBlock),
    Tnt(Tnt),
    Projectile(Projectile, ProjectileKind),
    Living(Living, LivingKind),
}

/// Kind of projectile entity.
#[derive(Debug, Clone)]
pub enum ProjectileKind {
    Arrow(Arrow),
    Egg(Egg),
    Fireball(Fireball),
    Snowball(Snowball),
}

/// Kind of living entity, this include animals and mobs.
#[derive(Debug, Clone)]
pub enum LivingKind {
    // Not categorized
    Human(Human),
    Ghast(Ghast),
    Slime(Slime),
    // Animal
    Pig(Pig),
    Chicken(Chicken),
    Cow(Cow),
    Sheep(Sheep),
    Squid(Squid),
    Wolf(Wolf),
    // Mob
    Creeper(Creeper),
    Giant(Giant),
    PigZombie(PigZombie),
    Skeleton(Skeleton),
    Spider(Spider),
    Zombie(Zombie),
}

/// The base data common to all entities.
#[derive(Debug, Clone, Default)]
pub struct Base {
    /// Tell if this entity is persistent or not. A persistent entity is saved with its
    /// chunk, but non-persistent entities are no saved. For example, all player entities
    /// are typically non-persistent because these are not real entities. Some entities
    /// cannot be persistent as they are not supported by the Notchian serialization.
    pub persistent: bool,
    /// Set to true when this entity is externally controlled.
    /// FIXME: This property is being tested.
    pub controlled: bool,
    /// Tell if the position of this entity and its bounding box are coherent, if false
    /// (the default value), this will recompute the bounding box from the center position
    /// and the size of the entity.
    pub coherent: bool,
    /// The last size that was used when recomputing the bounding box based on the 
    /// position, we keep it in order to check that the bounding box don't shift too far
    /// from it because of rounding errors, and also to keep the height center. This is
    /// updated with the bounding box by `tick_base` method when entity isn't coherent.
    pub size: Size,
    /// The bounding box is defining the actual position from the size of the entity, the 
    /// actual position of the entity is derived from it. This is recomputed with the size
    /// by `tick_base` method when entity isn't coherent.
    pub bb: BoundingBox,
    /// The current entity position, usually derived from the bounding box and size, it
    /// can be set forced by setting the size to none, this will force recomputation of
    /// the bounding box, instead of overwriting the position. The position is really
    /// important because it's used to properly cache the entity in its correct chunk,
    /// and properly do collision detection.
    pub pos: DVec3,
    /// True if an entity pos event should be sent after update.
    /// The current entity velocity.
    pub vel: DVec3,
    /// Yaw a pitch angles of this entity's look. These are in radians with no range 
    /// guarantee, although this will often be normalized in 2pi range. The yaw angle
    /// in Minecraft is set to zero when pointing toward PosZ, and then rotate clockwise
    /// to NegX, NegZ and then PosX.
    /// 
    /// Yaw is X and pitch is Y.
    pub look: Vec2,
    /// Lifetime of the entity since it was spawned in the world, it increase at every
    /// world tick.
    pub lifetime: u32,
    /// Height of the eyes, this is an Y offset from the position.
    pub eye_height: f32,
    /// Set to true when the entity is able to pickup surrounding items and arrows on
    /// ground, if so a pickup event is triggered, but the item or arrow is not actually
    /// picked up, it's up to the event listener to decide. Disabled by default.
    pub can_pickup: bool,
    /// No clip is used to disable collision check when moving the entity, if no clip is
    /// false, then the entity will be constrained by bounding box in its way.
    pub no_clip: bool,
    /// Is this entity currently on ground.
    pub on_ground: bool,
    /// Is this entity in water.
    pub in_water: bool,
    /// Is this entity in lava.
    pub in_lava: bool,
    /// True if the entity is immune to fire.
    pub fire_immune: bool,
    /// Total fall distance, will be used upon contact to calculate damages to deal.
    pub fall_distance: f32,
    /// Remaining fire ticks.
    pub fire_time: u32,
    /// Remaining air ticks to breathe.
    pub air_time: u32,
    /// A list of hurts to apply to the entity.
    pub hurt: Vec<Hurt>,
    /// If this entity is ridden, this contains its entity id.
    pub rider_id: Option<u32>,
    /// The random number generator used for this entity.
    pub rand: JavaRandom,
}

/// Hurt data to apply on the next tick to the entity.
#[derive(Debug, Clone, Default)]
pub struct Hurt {
    /// The damage to deal.
    pub damage: u16,
    /// When damage is dealt, this optionally contains the entity id at the origin of the
    /// hit in order to apply knock back to the entity if needed.
    pub origin_id: Option<u32>,
}

/// The data common to all living entities.
#[derive(Debug, Clone, Default)]
pub struct Living {
    /// The health.
    pub health: u16,
    /// The last damage inflicted to the entity during `hurt_time`, this is used to only
    /// damage for the maximum damage inflicted while `hurt_time` is not zero.
    pub hurt_last_damage: u16,
    /// Hurt countdown, read `hurt_damage` documentation.
    pub hurt_time: u16,
    /// TBD.
    pub attack_time: u16,
    /// The death timer, increasing each tick when no health, after 20 ticks the entity
    /// is definitely removed from the world.
    pub death_time: u16,
    /// The strafing acceleration.
    pub accel_strafing: f32,
    /// The forward acceleration.
    pub accel_forward: f32,
    /// Velocity of the look's yaw axis.
    pub yaw_velocity: f32,
    /// True if this entity is trying to jump.
    pub jumping: bool,
    /// If this entity is looking at another one.
    pub look_target: Option<LookTarget>,
    /// If this entity is attacking another one.
    pub attack_target: Option<u32>,
    /// The path this creature needs to follow.
    pub path: Option<Path>,
}

/// The data common to all projectile entities.
#[derive(Debug, Clone, Default)]
pub struct Projectile {
    /// The state of the projectile, none when in air, set to block/metadata when in.
    pub state: Option<ProjectileHit>,
    /// This is the number of ticks the projectile has been in its current state.
    pub state_time: u16,
    /// Some entity id if this projectile was thrown by an entity, this is used to avoid
    /// hitting the owner.
    pub owner_id: Option<u32>,
    /// Current shaking of the projectile.
    pub shake: u8,
}

#[derive(Debug, Copy, Clone, Default)]
pub struct ProjectileHit {
    /// The block position the projectile is in.
    pub pos: IVec3,
    /// The block the projectile is in.
    pub block: u8,
    /// The block metadata the projectile is in.
    pub metadata: u8,
}

#[derive(Debug, Clone, Default)]
pub struct Item {
    /// The item stack represented by this entity.
    pub stack: ItemStack,
    /// The item health.
    pub health: u16,
    /// Remaining time for this item to be picked up by entities that have `can_pickup`.
    pub frozen_time: u32,
}

#[derive(Debug, Clone, Default)]
pub struct Painting {
    /// Block position of this painting.
    pub block_pos: IVec3,
    /// Orientation of this painting at block position.
    pub orientation: PaintingOrientation,
    /// The art of the painting, which define its size.
    pub art: PaintingArt,
    /// This timer is used to repeatedly check if the painting is at a valid position.
    pub check_valid_time: u8,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum PaintingOrientation {
    #[default]
    NegX,
    PosX,
    NegZ,
    PosZ,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum PaintingArt {
    #[default]
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

#[derive(Debug, Clone, Default)]
pub struct Boat { }

#[derive(Debug, Clone, Default)]
pub enum Minecart { 
    /// A normal minecart for living entity transportation.
    #[default]
    Normal,
    /// A chest minecart for storing a single chest of items.
    Chest {
        /// The inventory storing the items.
        inv: Box<[ItemStack; 27]>,
    },
    /// A furnace minecart that push when fueled.
    Furnace {
        push_x: f64,
        push_z: f64,
        /// Remaining fuel amount.
        fuel: u32,
    }
}

#[derive(Debug, Clone, Default)]
pub struct Fish { }

#[derive(Debug, Clone, Default)]
pub struct LightningBolt { }

#[derive(Debug, Clone, Default)]
pub struct FallingBlock {
    /// Number of ticks since this block is falling.
    pub fall_ticks: u32,
    /// The falling block id.
    pub block_id: u8,
}

#[derive(Debug, Clone, Default)]
pub struct Tnt {
    pub fuse_time: u32,
}

#[derive(Debug, Clone, Default)]
pub struct Arrow {
    /// Set to true for arrows that are sent by players and therefore can be picked up.
    pub from_player: bool,
}

#[derive(Debug, Clone, Default)]
pub struct Egg { }

#[derive(Debug, Clone, Default)]
pub struct Fireball {
    /// Acceleration to that fireball.
    pub accel: DVec3,
}

#[derive(Debug, Clone, Default)]
pub struct Snowball { }

#[derive(Debug, Clone, Default)]
pub struct Human {
    /// The player username.
    pub username: String,
    /// True when the player is sleeping.
    pub sleeping: bool,
}

#[derive(Debug, Clone, Default)]
pub struct Ghast {
    /// The ghast waypoint defaults to zero.
    pub waypoint: DVec3,
    /// Remaining time before changing the target waypoint of the ghast.
    pub waypoint_check_time: u8,
    /// Remaining time before searching an attack target again.
    pub attack_target_time: u8,
}

#[derive(Debug, Clone, Default)]
pub struct Slime {
    /// Size of the slime.
    pub size: u8,
    /// Remaining time before jumping.
    pub jump_remaining_time: u32,
}

#[derive(Debug, Clone, Default)]
pub struct Pig {
    /// True when the pig has a saddle.
    pub saddle: bool,
}

#[derive(Debug, Clone, Default)]
pub struct Chicken {
    /// Ticks remaining until this chicken lays an egg.
    pub next_egg_ticks: u32,
}

#[derive(Debug, Clone, Default)]
pub struct Cow { }

#[derive(Debug, Clone, Default)]
pub struct Sheep {
    pub sheared: bool,
    pub color: u8, // TODO: Color enumeration.
}

#[derive(Debug, Clone, Default)]
pub struct Squid { }

#[derive(Debug, Clone, Default)]
pub struct Wolf {
    pub angry: bool,
    pub sitting: bool,
    pub owner: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct Creeper { 
    /// True when the creeper is powered.
    pub powered: bool,
    /// Set to some time when the creeper is ignited.
    pub ignited_time: Option<u16>
}

#[derive(Debug, Clone, Default)]
pub struct Giant { }

#[derive(Debug, Clone, Default)]
pub struct PigZombie { 
    pub anger: bool,
}

#[derive(Debug, Clone, Default)]
pub struct Skeleton { }

#[derive(Debug, Clone, Default)]
pub struct Spider { }

#[derive(Debug, Clone, Default)]
pub struct Zombie { }


/// Size of an entity, used to update each entity bounding box prior to ticking if 
/// relevant.
#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub struct Size {
    /// Width of the bounding box, centered on the X/Z coordinates.
    pub width: f32,
    /// Height of the bounding box.
    pub height: f32,
    /// Define the center of the bounding box on Y axis.
    pub center: f32,
}

impl Size {

    /// New size with the Y position at the bottom center of the bounding box.
    pub fn new(width: f32, height: f32) -> Self {
        Self { width, height, center: 0.0 }
    }

    /// New size with the Y position at the center of the bounding box.
    pub fn new_centered(width: f32, height: f32) -> Self {
        Self { width, height, center: height / 2.0 }
    }

}

/// Define a target for an entity to look at.
#[derive(Debug, Clone, Default)]
pub struct LookTarget {
    /// The entity id to look at.
    pub entity_id: u32,
    /// Ticks remaining before stop looking at it.
    pub remaining_time: u32,
}

/// A result of the path finder.
#[derive(Debug, Clone)]
pub struct Path {
    pub points: Vec<IVec3>,
    pub index: usize,
}

impl From<Vec<IVec3>> for Path {
    fn from(points: Vec<IVec3>) -> Self {
        Self { points, index: 0 }
    }
}

impl From<IVec3> for Path {
    fn from(value: IVec3) -> Self {
        Self { points: vec![value], index: 0 }
    }
}

impl Path {

    /// Return the current path position.
    pub fn point(&self) -> Option<IVec3> {
        self.points.get(self.index).copied()
    }

    /// Advanced the path by one point.
    pub fn advance(&mut self) {
        self.index += 1;
    }
    
}


impl EntityKind {

    /// Create a new default entity instance from the given type.
    pub fn new_default(self) -> Box<Entity> {
        
        use crate::util::default as def;

        Box::new(Entity(def(), match self {
            EntityKind::Item => BaseKind::Item(def()),
            EntityKind::Painting => BaseKind::Painting(def()),
            EntityKind::Boat => BaseKind::Boat(def()),
            EntityKind::Minecart => BaseKind::Minecart(def()),
            EntityKind::Fish => BaseKind::Fish(def()),
            EntityKind::LightningBolt => BaseKind::LightningBolt(def()),
            EntityKind::FallingBlock => BaseKind::FallingBlock(def()),
            EntityKind::Tnt => BaseKind::Tnt(def()),
            EntityKind::Arrow |
            EntityKind::Egg |
            EntityKind::Fireball |
            EntityKind::Snowball => {
                BaseKind::Projectile(def(), match self {
                    EntityKind::Arrow => ProjectileKind::Arrow(def()),
                    EntityKind::Egg => ProjectileKind::Egg(def()),
                    EntityKind::Fireball => ProjectileKind::Fireball(def()),
                    EntityKind::Snowball => ProjectileKind::Snowball(def()),
                    _ => unreachable!()
                })
            }
            _ => {
                BaseKind::Living(def(), match self {
                    EntityKind::Player => LivingKind::Human(def()),
                    EntityKind::Ghast => LivingKind::Ghast(def()),
                    EntityKind::Slime => LivingKind::Slime(def()),
                    EntityKind::Pig => LivingKind::Pig(def()),
                    EntityKind::Chicken => LivingKind::Chicken(def()),
                    EntityKind::Cow => LivingKind::Cow(def()),
                    EntityKind::Sheep => LivingKind::Sheep(def()),
                    EntityKind::Squid => LivingKind::Squid(def()),
                    EntityKind::Wolf => LivingKind::Wolf(def()),
                    EntityKind::Creeper => LivingKind::Creeper(def()),
                    EntityKind::Giant => LivingKind::Giant(def()),
                    EntityKind::PigZombie => LivingKind::PigZombie(def()),
                    EntityKind::Skeleton => LivingKind::Skeleton(def()),
                    EntityKind::Spider => LivingKind::Spider(def()),
                    EntityKind::Zombie => LivingKind::Zombie(def()),
                    _ => unreachable!()
                })
            }
        }))

    }

}


impl Entity {

    /// Get the kind of entity from this instance.
    pub fn kind(&self) -> EntityKind {
        self.1.entity_kind()
    }

    /// This this entity from its id in a world.
    #[instrument(level = "debug", skip_all)]
    pub fn tick(&mut self, world: &mut World, id: u32) {
        tick::tick(world, id, self);
    }

}

impl BaseKind {

    /// Get the generic entity kind from this base entity kind.
    pub fn entity_kind(&self) -> EntityKind {
        match self {
            BaseKind::Item(_) => EntityKind::Item,
            BaseKind::Painting(_) => EntityKind::Painting,
            BaseKind::Boat(_) => EntityKind::Boat,
            BaseKind::Minecart(_) => EntityKind::Minecart,
            BaseKind::Fish(_) => EntityKind::Fish,
            BaseKind::LightningBolt(_) => EntityKind::LightningBolt,
            BaseKind::FallingBlock(_) => EntityKind::FallingBlock,
            BaseKind::Tnt(_) => EntityKind::Tnt,
            BaseKind::Projectile(_, kind) => kind.entity_kind(),
            BaseKind::Living(_, kind) => kind.entity_kind(),
        }
    }

}

impl LivingKind {

    /// Get the generic entity kind from this living entity kind.
    pub fn entity_kind(&self) -> EntityKind {
        match self {
            LivingKind::Human(_) => EntityKind::Player,
            LivingKind::Ghast(_) => EntityKind::Ghast,
            LivingKind::Slime(_) => EntityKind::Slime,
            LivingKind::Pig(_) => EntityKind::Pig,
            LivingKind::Chicken(_) => EntityKind::Chicken,
            LivingKind::Cow(_) => EntityKind::Cow,
            LivingKind::Sheep(_) => EntityKind::Sheep,
            LivingKind::Squid(_) => EntityKind::Squid,
            LivingKind::Wolf(_) => EntityKind::Wolf,
            LivingKind::Creeper(_) => EntityKind::Creeper,
            LivingKind::Giant(_) => EntityKind::Giant,
            LivingKind::PigZombie(_) => EntityKind::PigZombie,
            LivingKind::Skeleton(_) => EntityKind::Skeleton,
            LivingKind::Spider(_) => EntityKind::Spider,
            LivingKind::Zombie(_) => EntityKind::Zombie,
        }
    }

}

impl ProjectileKind {

    /// Get the generic entity kind from this projectile entity kind.
    pub fn entity_kind(&self) -> EntityKind {
        match self {
            ProjectileKind::Arrow(_) => EntityKind::Arrow,
            ProjectileKind::Egg(_) => EntityKind::Egg,
            ProjectileKind::Fireball(_) => EntityKind::Fireball,
            ProjectileKind::Snowball(_) => EntityKind::Snowball,
        }
    }

}


macro_rules! impl_new_with {
    ( Base: $( $kind:ident ),* ) => {
        
        $(impl $kind {
            #[inline]
            pub fn new_with(func: impl FnOnce(&mut Base, &mut $kind)) -> Box<Entity> {
                let mut base: Base = Default::default();
                let mut this: $kind = Default::default();
                func(&mut base, &mut this);
                Box::new(Entity(base, BaseKind::$kind(this)))
            }
        })*

    };
    ( Living: $( $kind:ident ),* ) => {
        
        $(impl $kind {
            #[inline]
            pub fn new_with(func: impl FnOnce(&mut Base, &mut Living, &mut $kind)) -> Box<Entity> {
                let mut base: Base = Default::default();
                let mut living: Living = Default::default();
                let mut this: $kind = Default::default();
                func(&mut base, &mut living, &mut this);
                Box::new(Entity(base, BaseKind::Living(living, LivingKind::$kind(this))))
            }
        })*

    };
    ( Projectile: $( $kind:ident ),* ) => {
        
        $(impl $kind {
            #[inline]
            pub fn new_with(func: impl FnOnce(&mut Base, &mut Projectile, &mut $kind)) -> Box<Entity> {
                let mut base: Base = Default::default();
                let mut projectile: Projectile = Default::default();
                let mut this: $kind = Default::default();
                func(&mut base, &mut projectile, &mut this);
                Box::new(Entity(base, BaseKind::Projectile(projectile, ProjectileKind::$kind(this))))
            }
        })*

    };
}

impl_new_with!(Base: 
    Item, 
    Painting, 
    Boat, 
    Minecart, 
    Fish, 
    LightningBolt, 
    FallingBlock, 
    Tnt);

impl_new_with!(Living: 
    Human,
    Ghast,
    Slime,
    Pig,
    Chicken,
    Cow,
    Sheep,
    Squid,
    Wolf,
    Creeper,
    Giant,
    PigZombie,
    Skeleton,
    Spider,
    Zombie);
    
impl_new_with!(Projectile: 
    Arrow,
    Egg,
    Fireball,
    Snowball);
