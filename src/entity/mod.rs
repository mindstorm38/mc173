//! Entity data structures, no logic is defined here.

use std::ops::Add;
use std::any::Any;

use glam::{DVec3, Vec2};

use crate::block::{self, block_from_id, Material};
use crate::util::rand::JavaRandom;
use crate::util::bb::BoundingBox;
use crate::world::World;

mod falling_block;
mod player;
mod item;

pub use falling_block::FallingBlockEntity;
pub use player::PlayerEntity;
pub use item::ItemEntity;


/// Base class for entity.
#[derive(Debug)]
pub struct Base<I> {
    /// The internal entity id.
    pub id: u32,
    /// The current entity position.
    pub pos: DVec3,
    /// The current entity velocity.
    pub vel: DVec3,
    /// Yaw/Pitch look.
    pub look: Vec2,
    /// Is this entity responding to block's collisions.
    pub no_clip: bool,
    /// Is this entity currently on ground.
    pub on_ground: bool,
    /// Total fall distance, will be used upon contact to calculate damages to deal.
    pub fall_distance: f32,
    /// If this entity is ridden, this contains its entity id.
    pub rider_id: Option<u32>,
    /// Lifetime of the entity, in ticks.
    pub lifetime: u32,
    /// The health.
    pub health: u32,
    /// The random number generator used for this entity.
    pub random: JavaRandom,
    /// This bounding box is internally used by tick methods, it is usually initialized
    /// with [`update_bounding_box`] or [`update_entity`] methods.
    pub bounding_box: BoundingBox,
    /// Inner implementation of the entity.
    pub base: I,
}

impl<I: Default> Base<I> {

    pub fn new(pos: DVec3) -> Self {
        Self {
            id: 0,
            pos,
            vel: DVec3::ZERO,
            look: Vec2::ZERO,
            no_clip: false,
            on_ground: false,
            fall_distance: 0.0,
            rider_id: None,
            lifetime: 0,
            health: 1,
            random: JavaRandom::new_seeded(),
            bounding_box: BoundingBox::default(),
            base: I::default(),
        }
    }

}

impl<I> Base<I> {

    /// Update the internal bounding box depending on the entity position and given 
    /// bounding box size.
    pub fn update_bounding_box(&mut self, size: Size) {
        let half_width = (size.width / 2.0) as f64;
        let height = size.height as f64;
        let height_center = size.height_center as f64;
        self.bounding_box = BoundingBox {
            min: self.pos - DVec3::new(half_width, height_center, half_width),
            max: self.pos + DVec3::new(half_width, height + height_center, half_width),
        };
    }

    /// Common method to update entities..
    pub fn update_entity(&mut self, world: &mut World, size: Size) {

        self.lifetime += 1;
        self.update_bounding_box(size);

    }

    pub fn calc_fluid_velocity(&mut self, world: &mut World, material: Material) -> DVec3 {

        let fluid_bb = self.bounding_box.inflate(DVec3::new(-0.001, -0.4 - 0.001, -0.001));
        let min = fluid_bb.min.floor().as_ivec3();
        let max = fluid_bb.max.add(1.0).floor().as_ivec3();

        for (pos, block, metadata) in world.iter_area_blocks(min, max) {
            if block_from_id(block).material == material {

                let fluid_height = block::fluid::calc_fluid_height(metadata);
                let fluid_top_y = ((pos.y + 1) as f32 - fluid_height) as f64;

                if max.y as f64 >= fluid_top_y {
                    // block::fluid::calc_fluid_velocity(world, pos)
                }

            }
        }

        todo!()

    }





    /// Common tick function to apply the given gravity on the entity and move it, while
    /// managing block collisions.
    pub fn apply_gravity(&mut self, world: &mut World, step_height: f32) {
        self.vel.y -= 0.04;
        self.move_entity(world, self.vel, step_height);
        self.vel *= 0.98;
    }

    /// Common method for moving an entity by a given amount while checking collisions.
    pub fn move_entity(&mut self, world: &mut World, delta: DVec3, step_height: f32) {

        if self.no_clip {
            self.pos += delta;
        } else {

            // TODO: 

            // TODO: If in cobweb:
            // delta *= DVec3::new(0.25, 0.05, 0.25)
            // base.vel = DVec3::ZERO

            // TODO: Sneaking on ground

            let mut bb = self.bounding_box;
            let colliding_bbs: Vec<BoundingBox> = world.iter_colliding_bounding_boxes(bb.expand(delta))
                .collect();

            // Compute a new delta that doesn't collide with above boxes.
            let mut new_delta = delta;

            // Check collision on Y axis.
            for colliding_bb in &colliding_bbs {
                new_delta.y = colliding_bb.calc_y_delta(new_delta.y, bb);
            }

            bb += DVec3::new(0.0, new_delta.y, 0.0);

            // Check collision on X axis.
            for colliding_bb in &colliding_bbs {
                new_delta.x = colliding_bb.calc_x_delta(new_delta.x, bb);
            }

            bb += DVec3::new(new_delta.x, 0.0, 0.0);

            // Check collision on Z axis.
            for colliding_bb in &colliding_bbs {
                new_delta.z = colliding_bb.calc_z_delta(new_delta.z, bb);
            }
            
            bb += DVec3::new(0.0, 0.0, new_delta.z);
            let _ = bb; // No longer used because we calculated the final delta.

            let collided_x = delta.x != new_delta.x;
            let collided_y = delta.y != new_delta.y;
            let collided_z = delta.z != new_delta.z;
            let on_ground = collided_y && delta.y < 0.0; // || self.on_ground

            // Apply step if relevant.
            if step_height > 0.0 && on_ground && (collided_x || collided_z) {
                todo!("handle step motion");
            }

            self.pos += new_delta;
            self.on_ground = on_ground;

            if on_ground {
                if self.fall_distance > 0.0 {
                    // TODO: Damage?
                }
                self.fall_distance = 0.0;
            } else if new_delta.y < 0.0 {
                self.fall_distance -= new_delta.y as f32;
            }

            if collided_x {
                self.vel.x = 0.0;
            }

            if collided_y {
                self.vel.y = 0.0;
            }

            if collided_z {
                self.vel.z = 0.0;
            }

        }

    }

}


/// Base class for living entity.
#[derive(Debug, Default)]
pub struct Living<I> {
    /// The forward motion.
    pub move_forward: f32,
    /// The strafing motion.
    pub move_strafing: f32,
    /// True if this entity is trying to jump.
    pub jumping: bool,
    /// Inner implementation of the living entity.
    pub living: I,
}

impl<I> Base<Living<I>> {



}


/// Size of an entity, used when computing collisions and calculating new position.
#[derive(Debug, Clone, Copy)]
pub struct Size {
    pub width: f32,
    pub height: f32,
    pub height_center: f32,
}

impl Size {

    pub fn new(width: f32, height: f32) -> Self {
        Self { width, height, height_center: 0.0 }
    }

    pub fn new_centered(width: f32, height: f32) -> Self {
        Self { width, height, height_center: height / 2.0 }
    }

}


/// This trait can be used to implement entity logic. It also requires your type to also
/// implement the [`Any`] trait, this provides downcasts on dynamic pointers to entities.
pub trait EntityLogic: Any {

    /// Tick this entity and update its internal components.
    fn tick(&mut self, world: &mut World);

}


/// Base trait for [`EntityLogic`] implementors, it is automatically implemented for all
/// generic type [`Base`] and provides common methods to access base properties of an
/// entity behind a dynamic reference.
pub trait EntityGeneric: EntityLogic {

    /// Get the entity id.
    fn id(&self) -> u32;

    /// Get the entity position.
    fn pos(&self) -> DVec3;

    /// Get this entity as any type, this allows checking its real type.
    fn any(&self) -> &dyn Any;

    /// Get this entity as mutable any type.
    fn any_mut(&mut self) -> &mut dyn Any;

    /// Debug-purpose underlying type name.
    fn type_name(&self) -> &'static str;

}

impl dyn EntityGeneric {

    /// Check if this entity is of the given type.
    #[inline]
    pub fn is<E: EntityGeneric>(&self) -> bool {
        self.any().is::<E>()
    }

    #[inline]
    pub fn downcast_ref<E: EntityGeneric>(&self) -> Option<&E> {
        self.any().downcast_ref::<E>()
    }

    #[inline]
    pub fn downcast_mut<E: EntityGeneric>(&mut self) -> Option<&mut E> {
        self.any_mut().downcast_mut::<E>()
    }

}

impl<I> EntityGeneric for Base<I>
where
    I: 'static,
    Base<I>: EntityLogic,
{

    #[inline]
    fn id(&self) -> u32 {
        self.id
    }

    #[inline]
    fn pos(&self) -> DVec3 {
        self.pos
    }

    #[inline]
    fn any(&self) -> &dyn Any {
        self
    }

    #[inline]
    fn any_mut(&mut self) -> &mut dyn Any {
        self
    }

    #[inline]
    fn type_name(&self) -> &'static str {
        std::any::type_name::<Self>()
    }
    
}




/*
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
*/