//! Entity data structures, no logic is defined here.

use glam::{DVec3, Vec2};

use crate::util::rand::JavaRandom;
use crate::util::bb::BoundingBox;
use crate::world::World;

pub mod falling_block;


/// Base trait for implementing entity behaviors.
pub trait Entity {

    /// Tick this entity and update its internal components.
    fn tick(&mut self, world: &mut World);

    /// Get the base entity component that should be part of every entity.
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
    /// Is this entity responding to block's collisions.
    pub no_clip: bool,
    /// Is this entity currently on ground.
    pub on_ground: bool,
    /// The width of the entity's bounding box.
    pub width: f32,
    /// The height of the entity's bounding box.
    pub height: f32,
    /// Mark the center of the entity's bounding box in height.
    pub height_center: f32,
    /// The maximum step that can be taken by this entity when hitting a block 
    /// horizontally.
    pub step_height: f32,
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
}

impl BaseEntity {

    pub fn new(id: u32, pos: DVec3, width: f32, height: f32) -> Self {
        Self {
            id,
            pos,
            vel: DVec3::ZERO,
            look: Vec2::ZERO,
            no_clip: false,
            on_ground: false,
            width,
            height,
            height_center: 0.0,
            step_height: 0.0,
            fall_distance: 0.0,
            rider_id: None,
            lifetime: 0,
            health: 1,
            random: JavaRandom::new_seeded(),
        }
    }

    /// Calculate the bounding box of this entity, depending on its position, width and
    /// height and height offset.
    pub fn bounding_box(&self) -> BoundingBox {
        let half_width = (self.width / 2.0) as f64;
        let height = self.height as f64;
        let height_center = self.height_center as f64;
        BoundingBox {
            min: self.pos - DVec3::new(half_width, height_center, half_width),
            max: self.pos + DVec3::new(half_width, height + height_center, half_width),
        }
    }

    /// Common tick function to apply the given gravity on the entity and move it, while
    /// managing block collisions.
    pub fn apply_gravity(&mut self, world: &mut World) {
        self.vel.y -= 0.04;
        self.move_entity(world, self.vel);
        self.vel *= 0.98;
    }

    /// Common method for moving an entity by a given amount while checking collisions.
    pub fn move_entity(&mut self, world: &mut World, delta: DVec3) {

        if self.no_clip {
            self.pos += delta;
        } else {

            // TODO: 

            // TODO: If in cobweb:
            // delta *= DVec3::new(0.25, 0.05, 0.25)
            // base.vel = DVec3::ZERO

            // TODO: Sneaking on ground

            let mut bb = self.bounding_box();
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
            if self.step_height > 0.0 && on_ground && (collided_x || collided_z) {
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