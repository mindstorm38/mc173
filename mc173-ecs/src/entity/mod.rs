use bevy::app::{App, FixedUpdate, Plugin};
use bevy::math::{DVec3, IVec3, Vec2};
use bevy::ecs::component::Component;
use bevy::ecs::bundle::Bundle;
use bevy::ecs::entity::Entity;

use crate::geom::{BoundingBox, Face};
use crate::rand::JavaRandom;
use crate::item::ItemStack;

pub mod tick;


/// The ECS plugin registering all the entity systems and required resources.
pub struct EntityPlugin;

impl Plugin for EntityPlugin {

    fn build(&self, app: &mut App) {
        
        app.add_systems(FixedUpdate, tick::tick_all);

    }

}


/// Base common structure to all entities.
#[derive(Debug, Clone, Default, Component)]
pub struct Base {
    /// Tell if this entity is persistent or not. A persistent entity is saved with its
    /// chunk, but non-persistent entities are no saved. For example, all player entities
    /// are typically non-persistent because these are not real entities. Some entities
    /// cannot be persistent as they are not supported by the Notchian serialization.
    pub persistent: bool,
    /// Lifetime of the entity since it was spawned in the world, it increase at every
    /// world tick.
    pub lifetime: u32,
    /// The random number generator used for this entity.
    pub rand: JavaRandom,
}

/// An entity that can move with a bounding box in the world.
#[derive(Debug, Clone, Default, Component)]
pub struct Real {
    /// The current entity position, it is derived from the bounding box and size.
    pub bb: BoundingBox,
    /// The current entity position, it is derived from the bounding box and size.
    pub pos: DVec3,
    /// The velocity to apply on each tick to the position.
    pub vel: DVec3,
    /// True if the entity is able to no-clip when being applied its velocity.
    pub no_clip: bool,
    /// Is this entity currently on ground.
    pub on_ground: bool,
    /// Is this entity in water.
    pub in_water: bool,
    /// Is this entity in lava.
    pub in_lava: bool,
    /// Total fall distance, will be used upon contact to calculate damages to deal.
    pub fall_distance: f32,
}

/// An entity that is able to pickup items.
#[derive(Debug, Clone, Default, Component)]
pub struct Pickup;

/// For entities that have AI.
#[derive(Debug, Clone, Default, Component)]
pub struct Ai {
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
    pub attack_target: Option<Entity>,
    /// The path this creature needs to follow.
    pub path: Option<Path>,
}

/// A living entity.
#[derive(Debug, Clone, Default, Component)]
pub struct Living {
    /// The health.
    pub health: u16,
    /// Hurt countdown, read `hurt_damage` documentation.
    pub hurt_time: u16,
    /// TBD.
    pub attack_time: u16,
    /// The death timer, increasing each tick when no health, after 20 ticks the entity
    /// is definitely removed from the world.
    pub death_time: u16,
    /// The look of the entity head.
    pub look: Vec2,
    /// Height of the look, the is Y offset from position.
    pub look_height: f32,
}

/// A monster entity marker.
#[derive(Debug, Clone, Default, Component)]
pub struct Monster { }

/// A animal entity marker.
#[derive(Debug, Clone, Default, Component)]
pub struct Animal { }

/// A water animal entity marker.
#[derive(Debug, Clone, Default, Component)]
pub struct WaterAnimal { }

/// A projectile entity.
#[derive(Debug, Clone, Default, Component)]
pub struct Projectile {
    /// Some entity id if this projectile was thrown by an entity, this is used to avoid
    /// hitting the owner.
    pub owner: Option<Entity>,
    /// This is the number of ticks the projectile has been in its current state.
    pub time: u16,
}

/// A projectile that hit a block.
#[derive(Debug, Clone, Default, Component)]
pub struct WallProjectile {
    /// The block position the projectile is in.
    pub pos: IVec3,
    /// The block id the projectile is in.
    pub block_id: u8,
    /// The block metadata the projectile is in.
    pub block_metadata: u8,
    /// Current shaking of the projectile.
    pub shake: u8,
}

/// An item entity.
#[derive(Debug, Clone, Default, Component)]
pub struct Item {
    /// The item stack represented by this entity.
    pub stack: ItemStack,
    /// The item health.
    pub health: u16,
    /// Remaining time for this item to be picked up by entities that have `can_pickup`.
    pub frozen_time: u32,
}

/// A painting entity.
#[derive(Debug, Clone, Component)]
pub struct Painting {
    /// Block position of this painting.
    pub block_pos: IVec3,
    /// The face of the block position the painting is on. Should not be on Y axis.
    pub face: Face,
    /// The art of the painting, which define its size.
    pub art: PaintingArt,
    /// This timer is used to repeatedly check if the painting is at a valid position.
    pub check_valid_time: u8,
}

impl Default for Painting {
    fn default() -> Self {
        Self { 
            block_pos: Default::default(),
            face: Face::NegX, 
            art: Default::default(), 
            check_valid_time: Default::default(),
        }
    }
}

/// A boat entity.
#[derive(Debug, Clone, Default, Component)]
pub struct Boat { }

/// Base component for all minecart entities.
#[derive(Debug, Clone, Default, Component)]
pub struct Minecart {}

/// Minecart chest.
#[derive(Debug, Clone, Default, Component)]
pub struct MinecartChest {
    /// The inventory storing the items.
    pub inv: Box<[ItemStack; 27]>,
}

/// Minecart furnace.
#[derive(Debug, Clone, Default, Component)]
pub struct MinecartFurnace {
    pub push_x: f64,
    pub push_z: f64,
    /// Remaining fuel amount.
    pub fuel: u32,
}

/// A fishing bobber.
#[derive(Debug, Clone, Default, Component)]
pub struct Bobber { 
    /// Some entity id if this bobber is attached to an entity instead of floating in 
    /// water.
    pub attached_entity: Option<Entity>,
    /// The remaining time for the bobber to be caught and have a chance of getting a 
    /// fish.
    pub catch_time: u16,
}

/// A lightning bolt entity.
#[derive(Debug, Clone, Default, Component)]
pub struct LightningBolt { }

/// A falling block entity.
#[derive(Debug, Clone, Default, Component)]
pub struct FallingBlock {
    /// Number of ticks since this block is falling.
    pub fall_time: u32,
    /// The falling block id.
    pub block_id: u8,
}

/// A primed TnT entity.
#[derive(Debug, Clone, Default, Component)]
pub struct Tnt {
    pub fuse_time: u32,
}

/// Arrow projectile.
#[derive(Debug, Clone, Default, Component)]
pub struct Arrow {
    /// Set to true for arrows that are sent by players and therefore can be picked up.
    pub from_player: bool,
}

/// Arrow projectile.
#[derive(Debug, Clone, Default, Component)]
pub struct Egg { }

/// A fireball entity.
#[derive(Debug, Clone, Default, Component)]
pub struct Fireball {
    /// Acceleration to that fireball.
    pub accel: DVec3,
}

/// A snowball entity.
#[derive(Debug, Clone, Default, Component)]
pub struct Snowball { }

/// A human entity.
#[derive(Debug, Clone, Default, Component)]
pub struct Human {
    /// The player username.
    pub username: String,
    /// True when the player is sleeping.
    pub sleeping: bool,
    /// True when the player is sneaking.
    pub sneaking: bool,
}

/// A ghast entity.
#[derive(Debug, Clone, Default, Component)]
pub struct Ghast {
    /// The ghast waypoint defaults to zero.
    pub waypoint: DVec3,
    /// Remaining time before changing the target waypoint of the ghast.
    pub waypoint_check_time: u8,
    /// Remaining time before searching an attack target again.
    pub attack_target_time: u8,
}

/// A slime entity.
#[derive(Debug, Clone, Default, Component)]
pub struct Slime {
    /// Size of the slime, this is a bit different because here the size is initially 
    /// at 0 and this is equivalent to 1 in Notchian implementation.
    pub size: u8,
    /// Remaining time before jumping.
    pub jump_remaining_time: u32,
}

/// A pig entity.
#[derive(Debug, Clone, Default, Component)]
pub struct Pig {
    /// True when the pig has a saddle.
    pub saddle: bool,
}

/// A chicken entity.
#[derive(Debug, Clone, Default, Component)]
pub struct Chicken {
    /// Ticks remaining until this chicken lays an egg.
    pub next_egg_ticks: u32,
}

/// A cow entity.
#[derive(Debug, Clone, Default, Component)]
pub struct Cow { }

/// A sheep entity.
#[derive(Debug, Clone, Default, Component)]
pub struct Sheep {
    pub sheared: bool,
    pub color: u8, // TODO: Color enumeration.
}

/// A squid entity.
#[derive(Debug, Clone, Default, Component)]
pub struct Squid {
    /// Animation progress for the squid.
    pub animation: f32,
    /// Speed of the animation.
    pub animation_speed: f32,
}

/// A wolf entity.
#[derive(Debug, Clone, Default, Component)]
pub struct Wolf {
    pub angry: bool,
    pub sitting: bool,
    pub owner_name: Option<String>,
    pub owner: Option<Entity>,
}

/// A creeper entity.
#[derive(Debug, Clone, Default, Component)]
pub struct Creeper { 
    /// True when the creeper is powered.
    pub powered: bool,
    /// Set to some time when the creeper is ignited.
    pub ignited_time: Option<u16>
}

/// A giant entity.
#[derive(Debug, Clone, Default, Component)]
pub struct Giant { }

/// A pig zombie entity.
#[derive(Debug, Clone, Default, Component)]
pub struct PigZombie { 
    pub anger: bool,
}

/// A skeleton entity.
#[derive(Debug, Clone, Default, Component)]
pub struct Skeleton { }

/// A spider entity.
#[derive(Debug, Clone, Default, Component)]
pub struct Spider { }

/// A zombie entity.
#[derive(Debug, Clone, Default, Component)]
pub struct Zombie { }

macro_rules! def_bundle {
    ( $( $name:ident { $( $key:ident : $comp:ty ),* $(,)? } )* ) => {
        $(
            #[derive(Default, Bundle)]
            pub struct $name {
                pub base: Base,
                $( pub $key: $comp, )*
            }
        )*
    };
    ( "real" $( $name:ident { $( $key:ident : $comp:ty ),* $(,)? } )* ) => {
        def_bundle!( $( $name { real: Real, $( $key: $comp ),* } )* );
    };
    ( "projectile" $( $name:ident { $( $key:ident : $comp:ty ),* $(,)? } )* ) => {
        def_bundle!( $( $name { real: Real, projectile: Projectile, $( $key: $comp ),* } )* );
    };
    ( "living" $( $name:ident { $( $key:ident : $comp:ty ),* $(,)? } )* ) => {
        def_bundle!( $( $name { real: Real, living: Living, $( $key: $comp ),* } )* );
    };
}

def_bundle! {
    PaintingBundle { painting: Painting, }
    LightningBoltBundle { lightning_bolt: LightningBolt, }
}

def_bundle! {
    "real"
    ItemBundle { item: Item, }
    BoatBundle { boat: Boat, }
    MinecartBundle { minecraft: Minecart, }
    MinecartChestBundle { minecart: Minecart, chest: MinecartChest, }
    MinecartFurnaceBundle { minecart: Minecart, furnace: MinecartFurnace, }
    BobberBundle { bobber: Bobber, }
    FallingBlockBundle { falling_block: FallingBlock, }
    TntBundle { tnt: Tnt, }
}

def_bundle! {
    "projectile"
    ArrowBundle { arrow: Arrow, }
    EggBundle { egg: Egg, }
    FireballBundle { fireball: Fireball, }
    SnowballBundle { snowball: Snowball, }
}

def_bundle! {
    "living"
    HumanBundle { human: Human, }
    GhastBundle { ghast: Ghast, }
    SlimeBundle { slime: Slime, monster: Monster, }
    PigBundle { pig: Pig, animal: Animal, }
    ChickenBundle { chicken: Chicken, animal: Animal, }
    CowBundle { cow: Cow, animal: Animal, }
    SheepBundle { sheep: Sheep, animal: Animal, }
    SquidBundle { squid: Squid, water: WaterAnimal, }
    WolfBundle { wolf: Wolf, animal: Animal, }
    CreeperBundle { creeper: Creeper, monster: Monster, }
    GiantBundle { giant: Giant, monster: Monster, }
    PigZombieBundle { pig_zombie: PigZombie, monster: Monster, }
    SkeletonBundle { skeleton: Skeleton, monster: Monster, }
    SpiderBundle { spider: Spider, monster: Monster, }
    ZombieBundle { zombie: Zombie, monster: Monster, }
}


/// Define a target for an entity to look at.
#[derive(Debug, Clone)]
pub struct LookTarget {
    /// The entity id to look at.
    pub entity: Entity,
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

/// Represent the art type for a painting.
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

impl PaintingArt {

    pub const ALL: [PaintingArt; 25] = [
        Self::Kebab,
        Self::Aztec,
        Self::Alban,
        Self::Aztec2,
        Self::Bomb,
        Self::Plant,
        Self::Wasteland,
        Self::Pool,
        Self::Courbet,
        Self::Sea,
        Self::Sunset,
        Self::Creebet,
        Self::Wanderer,
        Self::Graham,
        Self::Match,
        Self::Bust,
        Self::Stage,
        Self::Void,
        Self::SkullAndRoses,
        Self::Fighters,
        Self::Pointer,
        Self::Pigscene,
        Self::BurningSkull,
        Self::Skeleton,
        Self::DonkeyKong,
    ];

    /// Return the size of the painting, in blocks (width, height).
    pub const fn size(self) -> (u8, u8) {
        match self {
            Self::Kebab => (1, 1),
            Self::Aztec => (1, 1),
            Self::Alban => (1, 1),
            Self::Aztec2 => (1, 1),
            Self::Bomb => (1, 1),
            Self::Plant => (1, 1),
            Self::Wasteland => (1, 1),
            Self::Pool => (2, 1),
            Self::Courbet => (2, 1),
            Self::Sea => (2, 1),
            Self::Sunset => (2, 1),
            Self::Creebet => (2, 1),
            Self::Wanderer => (1, 2),
            Self::Graham => (1, 2),
            Self::Match => (2, 2),
            Self::Bust => (2, 2),
            Self::Stage => (2, 2),
            Self::Void => (2, 2),
            Self::SkullAndRoses => (2, 2),
            Self::Fighters => (4, 2),
            Self::Pointer => (4, 4),
            Self::Pigscene => (4, 4),
            Self::BurningSkull => (4, 4),
            Self::Skeleton => (4, 3),
            Self::DonkeyKong => (4, 3),
        }
    }

}
