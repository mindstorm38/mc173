//! Entity data structures, no logic is defined here.

use derive_more::{Deref, DerefMut};
use glam::{DVec3, Vec2, IVec3};

use crate::util::{JavaRandom, BoundingBox};
use crate::item::inventory::Inventory;
use crate::item::ItemStack;
use crate::world::World;

pub mod base;
pub mod item;
pub mod living;
pub mod pig;
pub mod player;
pub mod falling_block;


pub type ItemEntity = Base<Item>;
pub type PaintingEntity = Base<Painting>;
pub type BoatEntity = Base<Boat>;
pub type MinecartEntity = Base<Minecart>;
pub type FishEntity = Base<Fish>;
pub type LightningBoltEntity = Base<LightningBolt>;
pub type FallingBlockEntity = Base<FallingBlock>;
pub type TntEntity = Base<Tnt>;

type ProjectileEntity<I> = Base<Projectile<I>>;
pub type ArrowEntity = ProjectileEntity<Arrow>;
pub type EggEntity = ProjectileEntity<Egg>;
pub type FireballEntity = ProjectileEntity<Fireball>;
pub type SnowballEntity = ProjectileEntity<Snowball>;

type LivingEntity<I> = Base<Living<I>>;
pub type PlayerEntity = LivingEntity<Player>;
pub type GhastEntity = LivingEntity<Ghast>;
pub type SlimeEntity = LivingEntity<Slime>;

pub type PigEntity = LivingEntity<Pig>;
pub type ChickenEntity = LivingEntity<Chicken>;
pub type CowEntity = LivingEntity<Cow>;
pub type SheepEntity = LivingEntity<Sheep>;
pub type SquidEntity = LivingEntity<Squid>;
pub type WolfEntity = LivingEntity<Wolf>;

pub type CreeperEntity = LivingEntity<Creeper>;
pub type GiantEntity = LivingEntity<Giant>;
pub type PigZombieEntity = LivingEntity<PigZombie>;
pub type SkeletonEntity = LivingEntity<Skeleton>;
pub type SpiderEntity = LivingEntity<Spider>;
pub type ZombieEntity = LivingEntity<Zombie>;


/// This is an enumeration of all entities supported by the game, this enumeration allows
/// dispatching calls to update function and ensures that required functions gets called.
#[derive(Debug)]
pub enum Entity {
    Item(ItemEntity),
    Painting(PaintingEntity),
    Boat(BoatEntity),
    Minecart(MinecartEntity),
    Fish(FishEntity),
    LightningBolt(LightningBoltEntity),
    FallingBlock(FallingBlockEntity),
    Tnt(TntEntity),
    Arrow(ArrowEntity),
    Egg(EggEntity),
    Fireball(FireballEntity),
    Snowball(SnowballEntity),
    Player(PlayerEntity),
    Ghast(GhastEntity),
    Slime(SlimeEntity),
    Pig(PigEntity),
    Chicken(ChickenEntity),
    Cow(CowEntity),
    Sheep(SheepEntity),
    Squid(SquidEntity),
    Wolf(WolfEntity),
    Creeper(CreeperEntity),
    Giant(GiantEntity),
    PigZombie(PigZombieEntity),
    Skeleton(SkeletonEntity),
    Spider(SpiderEntity),
    Zombie(ZombieEntity),
}

#[derive(Debug, Default, Deref, DerefMut)]
pub struct Base<I> {
    /// Inner data.
    #[deref]
    #[deref_mut]
    pub data: BaseData,
    /// Inner implementation of the entity.
    pub kind: I,
}

#[derive(Debug, Default)]
pub struct BaseData {
    /// The internal entity id, it is unique to this entity within its world. It can be
    /// used to uniquely refer to this entity when and where needed.
    pub id: u32,
    /// This property is set to true when the entity is dead and should be removed from
    /// the world. This is the only property that controls despawning of entities. Note
    /// that the dead entity will just disappear, without any drop.
    pub dead: bool,
    /// Tell if the position of this entity and its bounding box are coherent, if false
    /// (the default value), this will recompute the bounding box from the center position
    /// and the size given to `tick_base` method.
    pub coherent: bool,
    /// The last size that was used when recomputing the bounding box based on the 
    /// position, we keep it in order to check that the bounding box don't shift to far
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
    pub look: Vec2,
    /// True if an entity look event should be sent after update.
    pub look_dirty: bool,
    /// Lifetime of the entity since it was spawned in the world, it increase at every
    /// world tick.
    pub lifetime: u32,
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
    /// The health.
    pub health: u32,
    /// If this entity is ridden, this contains its entity id.
    pub rider_id: Option<u32>,
    /// The random number generator used for this entity.
    pub rand: JavaRandom,
}

#[derive(Debug, Default, Deref, DerefMut)]
pub struct Living<I> {
    /// Inner data.
    #[deref]
    #[deref_mut]
    pub data: LivingData,
    /// Inner implementation of the living entity.
    pub kind: I,
}

#[derive(Debug, Default)]
pub struct LivingData {
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

#[derive(Debug, Default, Deref, DerefMut)]
pub struct Projectile<I> {
    /// Inner data.
    #[deref]
    #[deref_mut]
    pub data: ProjectileData,
    /// Projectile specialized structure.
    pub kind: I,
}

#[derive(Debug, Default)]
pub struct ProjectileData {
    /// Set to the position and block id this projectile is stuck in.
    pub block_hit: Option<(IVec3, u8)>,
    /// Some entity id if this projectile was thrown by an entity.
    pub owner_id: Option<u32>,
}

#[derive(Debug, Default)]
pub struct Item {
    /// The item stack represented by this entity.
    pub stack: ItemStack,
    /// Tick count before this item entity can be picked up.
    pub frozen_ticks: u32,
}

#[derive(Debug, Default)]
pub struct Painting {
    /// Block position of this painting.
    pub block_pos: IVec3,
    /// Orientation of this painting at block position.
    pub orientation: PaintingOrientation,
    /// The art of the painting, which define its size.
    pub art: PaintingArt,
}

#[derive(Debug, Default)]
pub struct Boat { }

#[derive(Debug, Default)]
pub struct Minecart { }

#[derive(Debug, Default)]
pub struct Fish { }

#[derive(Debug, Default)]
pub struct LightningBolt { }

#[derive(Debug, Default)]
pub struct FallingBlock {
    /// Number of ticks since this block is falling.
    pub fall_ticks: u32,
    /// The falling block id.
    pub block_id: u8,
}

#[derive(Debug, Default)]
pub struct Tnt {
    pub fuse_ticks: u32,
}

#[derive(Debug, Default)]
pub struct Arrow { }

#[derive(Debug, Default)]
pub struct Egg { }

#[derive(Debug, Default)]
pub struct Fireball { }

#[derive(Debug, Default)]
pub struct Snowball { }

#[derive(Debug)]
pub struct Player {
    /// The player username.
    pub username: String,
    /// True when the player is sleeping.
    pub sleeping: bool,
    /// Main inventory, with 36 slots.
    pub main_inv: Inventory,
    /// Armor inventory, with 4 slots.
    pub armor_inv: Inventory,
    /// The crafting matrix inventory with 4 slots.
    pub craft_inv: Inventory,
    /// Current item stack being hold by the player's window cursor, placed here in order
    /// to drop it when the player dies.
    pub cursor_stack: ItemStack,
    /// Index of the slot selected in the hotbar.
    pub hand_slot: u8,
}

impl Default for Player {
    fn default() -> Self {
        Self { 
            username: Default::default(), 
            sleeping: false, 
            main_inv: Inventory::new(36),
            armor_inv: Inventory::new(4), 
            craft_inv: Inventory::new(4),
            cursor_stack: ItemStack::EMPTY, 
            hand_slot: 0,
        }
    }
}

#[derive(Debug, Default)]
pub struct Ghast { }

#[derive(Debug, Default)]
pub struct Slime { }

#[derive(Debug, Default)]
pub struct Pig {
    /// True when the pig has a saddle.
    pub saddle: bool,
}

#[derive(Debug, Default)]
pub struct Chicken {
    /// Ticks remaining until this chicken lays an egg.
    pub next_egg_ticks: u32,
}

#[derive(Debug, Default)]
pub struct Cow { }

#[derive(Debug, Default)]
pub struct Sheep {
    pub color: u8, // TODO: Color enumeration.
}

#[derive(Debug, Default)]
pub struct Squid { }

#[derive(Debug, Default)]
pub struct Wolf {
    /// Entity owning the wolf.
    pub owner_id: u32,
}

#[derive(Debug, Default)]
pub struct Creeper { }

#[derive(Debug, Default)]
pub struct Giant { }

#[derive(Debug, Default)]
pub struct PigZombie { }

#[derive(Debug, Default)]
pub struct Skeleton { }

#[derive(Debug, Default)]
pub struct Spider { }

#[derive(Debug, Default)]
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
#[derive(Debug, Default)]
pub struct LookTarget {
    /// The entity id to look at.
    pub entity_id: u32,
    /// Ticks remaining before stop looking at it.
    pub ticks_remaining: u32,
}


/// A result of the path finder.
#[derive(Debug)]
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


#[derive(Debug, Default)]
pub enum PaintingOrientation {
    #[default]
    North,
    East,
    South,
    West,
}

#[derive(Debug, Default)]
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


// These verbose methods are intentionally placed at the end.
impl Entity {

    /// Tick the entity.
    pub fn tick(&mut self, world: &mut World) {
        match self {
            Entity::Item(base) => base.tick_item(world),
            Entity::Painting(base) => base.tick_base(world, Size::new(0.5, 0.5)),
            Entity::Boat(base) => base.tick_base(world, Size::new_centered(1.5, 0.6)),
            Entity::Minecart(base) => base.tick_base(world, Size::new_centered(0.98, 0.7)),
            Entity::Fish(base) => base.tick_base(world, Size::default()),
            Entity::LightningBolt(base) => base.tick_base(world, Size::default()),
            Entity::FallingBlock(base) => base.tick_falling_block(world),
            Entity::Tnt(base) => base.tick_base(world, Size::new_centered(0.98, 0.98)),
            Entity::Arrow(base) => base.tick_base(world, Size::new(0.5, 0.5)),
            Entity::Egg(base) => base.tick_base(world, Size::new(0.25, 0.25)),
            Entity::Fireball(base) => base.tick_base(world, Size::new(1.0, 1.0)),
            Entity::Snowball(base) => base.tick_base(world, Size::new(0.25, 0.25)),
            Entity::Player(base) => base.tick_player(world),
            Entity::Ghast(base) => base.tick_base(world, Size::new(4.0, 4.0)),
            Entity::Slime(base) => base.tick_base(world, Size::new(0.6, 0.6)),  // NOTE: Small slime size.
            Entity::Pig(base) => base.tick_pig(world),
            Entity::Chicken(base) => base.tick_base(world, Size::new(0.3, 0.4)),
            Entity::Cow(base) => base.tick_base(world, Size::new(0.9, 1.3)),
            Entity::Sheep(base) => base.tick_base(world, Size::new(0.9, 1.3)),
            Entity::Squid(base) => base.tick_base(world, Size::new(0.95, 0.95)),
            Entity::Wolf(base) => base.tick_base(world, Size::new(0.8, 0.8)),
            Entity::Creeper(base) => base.tick_base(world, Size::new(0.6, 1.8)),
            Entity::Giant(base) => base.tick_base(world, Size::new(3.6, 10.8)),
            Entity::PigZombie(base) => base.tick_base(world, Size::new(0.6, 1.8)),
            Entity::Skeleton(base) => base.tick_base(world, Size::new(0.6, 1.8)),
            Entity::Spider(base) => base.tick_base(world, Size::new(1.4, 0.9)),
            Entity::Zombie(base) => base.tick_base(world, Size::new(0.6, 1.8)),
        }
    }

    /// Immutable access to the base data.
    pub fn base(&self) -> &BaseData {
        match self {
            Entity::Item(base) => &base.data,
            Entity::Painting(base) => &base.data,
            Entity::Boat(base) => &base.data,
            Entity::Minecart(base) => &base.data,
            Entity::Fish(base) => &base.data,
            Entity::LightningBolt(base) => &base.data,
            Entity::FallingBlock(base) => &base.data,
            Entity::Tnt(base) => &base.data,
            Entity::Arrow(base) => &base.data,
            Entity::Egg(base) => &base.data,
            Entity::Fireball(base) => &base.data,
            Entity::Snowball(base) => &base.data,
            Entity::Player(base) => &base.data,
            Entity::Ghast(base) => &base.data,
            Entity::Slime(base) => &base.data,
            Entity::Pig(base) => &base.data,
            Entity::Chicken(base) => &base.data,
            Entity::Cow(base) => &base.data,
            Entity::Sheep(base) => &base.data,
            Entity::Squid(base) => &base.data,
            Entity::Wolf(base) => &base.data,
            Entity::Creeper(base) => &base.data,
            Entity::Giant(base) => &base.data,
            Entity::PigZombie(base) => &base.data,
            Entity::Skeleton(base) => &base.data,
            Entity::Spider(base) => &base.data,
            Entity::Zombie(base) => &base.data,
        }
    }

    /// Immutable access to the base data.
    pub fn base_mut(&mut self) -> &mut BaseData {
        match self {
            Entity::Item(base) => &mut base.data,
            Entity::Painting(base) => &mut base.data,
            Entity::Boat(base) => &mut base.data,
            Entity::Minecart(base) => &mut base.data,
            Entity::Fish(base) => &mut base.data,
            Entity::LightningBolt(base) => &mut base.data,
            Entity::FallingBlock(base) => &mut base.data,
            Entity::Tnt(base) => &mut base.data,
            Entity::Arrow(base) => &mut base.data,
            Entity::Egg(base) => &mut base.data,
            Entity::Fireball(base) => &mut base.data,
            Entity::Snowball(base) => &mut base.data,
            Entity::Player(base) => &mut base.data,
            Entity::Ghast(base) => &mut base.data,
            Entity::Slime(base) => &mut base.data,
            Entity::Pig(base) => &mut base.data,
            Entity::Chicken(base) => &mut base.data,
            Entity::Cow(base) => &mut base.data,
            Entity::Sheep(base) => &mut base.data,
            Entity::Squid(base) => &mut base.data,
            Entity::Wolf(base) => &mut base.data,
            Entity::Creeper(base) => &mut base.data,
            Entity::Giant(base) => &mut base.data,
            Entity::PigZombie(base) => &mut base.data,
            Entity::Skeleton(base) => &mut base.data,
            Entity::Spider(base) => &mut base.data,
            Entity::Zombie(base) => &mut base.data,
        }
    }

}
