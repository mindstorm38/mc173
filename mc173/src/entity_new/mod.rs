//! Trying a new architecture for entity structure (again!!!).

use glam::{DVec3, Vec2, IVec3};

use crate::item::ItemStack;
use crate::util::{BoundingBox, JavaRandom};


/// Base type that contains all entity types.
#[derive(Debug, Clone)]
pub struct Entity(pub Base, pub BaseKind);

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

#[derive(Debug, Clone)]
pub enum ProjectileKind {
    Arrow(Arrow),
    Egg(Egg),
    Fireball(Fireball),
    Snowball(Snowball),
}

#[derive(Debug, Clone)]
pub enum LivingKind {
    // Not categorized
    Player(Player),
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

#[derive(Debug, Clone, Default)]
pub struct Base {
    /// Tell if this entity is persistent or not. A persistent entity is saved with its
    /// chunk, but non-persistent entities are no saved. For example, all player entities
    /// are typically non-persistent because these are not real entities.
    pub persistent: bool,
    /// Tell if the position of this entity and its bounding box are coherent, if false
    /// (the default value), this will recompute the bounding box from the center position
    /// and the size given to `tick_base` method.
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
    /// 
    /// TODO: Maybe replace this by a special wrapper type around pos and look, and maybe 
    /// other properties in the future...
    pub pos_dirty: bool,
    /// The current entity velocity.
    pub vel: DVec3,
    /// True if an entity velocity event should be sent after update.
    pub vel_dirty: bool,
    /// Yaw a pitch angles of this entity's look. These are in radians with no range 
    /// guarantee, although this will often be normalized in 2pi range. The yaw angle
    /// in Minecraft is set to zero when pointing toward PosZ, and then rotate clockwise
    /// to NegX, NegZ and then PosX.
    /// 
    /// Yaw is X and pitch is Y.
    pub look: Vec2,
    /// True if an entity look event should be sent after update.
    pub look_dirty: bool,
    /// Lifetime of the entity since it was spawned in the world, it increase at every
    /// world tick.
    pub lifetime: u32,
    /// Set to true when the entity is able to pickup surrounding items and arrows on
    /// ground, if so a pickup event is triggered, but the item or arrow is not actually
    /// picked up, it's up to the event listener to decide. Disabled by default.
    /// TODO: Make it work.
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
    /// Total fall distance, will be used upon contact to calculate damages to deal.
    pub fall_distance: f32,
    /// Remaining fire ticks.
    pub fire_ticks: u32,
    /// Remaining air ticks to breathe.
    pub air_ticks: u32,
    /// The health.
    pub health: u32,
    /// If this entity is ridden, this contains its entity id.
    pub rider_id: Option<u32>,
    /// The random number generator used for this entity.
    pub rand: JavaRandom,
}

#[derive(Debug, Clone, Default)]
pub struct Living {
    /// The strafing acceleration.
    pub accel_strafing: f32,
    /// The forward acceleration.
    pub accel_forward: f32,
    /// Velocity of the look's yaw axis.
    pub yaw_velocity: f32,
    /// True if this entity is trying to jump.
    pub jumping: bool,
    /// If this entity can attack others, this defines its attack strength.
    pub attack_strength: i32,
    /// If this entity is looking at another one.
    pub look_target: Option<LookTarget>,
    /// The path this creature needs to follow.
    pub path: Option<Path>,
}

#[derive(Debug, Clone, Default)]
pub struct Projectile {
    /// Set to the position and block id this projectile is stuck in.
    pub block_hit: Option<(IVec3, u8, u8)>,
    /// Some entity id if this projectile was thrown by an entity.
    pub owner_id: Option<u32>,
    /// Current shaking of the projectile.
    pub shake: u8,
}

#[derive(Debug, Clone, Default)]
pub struct Item {
    /// The item stack represented by this entity.
    pub stack: ItemStack,
    /// Tick count before this item entity can be picked up.
    pub frozen_ticks: u32,
}

#[derive(Debug, Clone, Default)]
pub struct Painting {
    /// Block position of this painting.
    pub block_pos: IVec3,
    /// Orientation of this painting at block position.
    pub orientation: PaintingOrientation,
    /// The art of the painting, which define its size.
    pub art: PaintingArt,
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
    pub fuse_ticks: u32,
}

#[derive(Debug, Clone, Default)]
pub struct Arrow { }

#[derive(Debug, Clone, Default)]
pub struct Egg { }

#[derive(Debug, Clone, Default)]
pub struct Fireball { }

#[derive(Debug, Clone, Default)]
pub struct Snowball { }

#[derive(Debug, Clone, Default)]
pub struct Player {
    /// The player username.
    pub username: String,
    /// True when the player is sleeping.
    pub sleeping: bool,
}

#[derive(Debug, Clone, Default)]
pub struct Ghast { }

#[derive(Debug, Clone, Default)]
pub struct Slime {
    /// Size of the slime.
    pub size: u8,
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
    pub powered: bool,
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
    pub height_center: f32,
}

impl Size {

    /// New size with the Y position at the bottom center of the bounding box.
    pub fn new(width: f32, height: f32) -> Self {
        Self { width, height, height_center: 0.0 }
    }

    /// New size with the Y position at the center of the bounding box.
    pub fn new_centered(width: f32, height: f32) -> Self {
        Self { width, height, height_center: height / 2.0 }
    }

}

/// Define a target for an entity to look at.
#[derive(Debug, Clone, Default)]
pub struct LookTarget {
    /// The entity id to look at.
    pub entity_id: u32,
    /// Ticks remaining before stop looking at it.
    pub ticks_remaining: u32,
}

/// A result of the path finder.
#[derive(Debug, Clone)]
pub struct Path {
    pub points: Vec<IVec3>,
    pub index: usize,
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

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum PaintingOrientation {
    #[default]
    NegX,
    PosX,
    NegZ,
    PosZ,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
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
