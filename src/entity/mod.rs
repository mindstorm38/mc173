//! Entity data structures, no logic is defined here.

use std::ops::Add;
use std::any::Any;

use glam::{DVec3, Vec2, IVec3};

use crate::block::{self, block_from_id, Material};
use crate::util::rand::JavaRandom;
use crate::util::bb::BoundingBox;
use crate::world::{World, Event};

mod falling_block;
mod player;
mod item;
mod pig;

pub use falling_block::FallingBlockEntity;
pub use player::PlayerEntity;
pub use item::ItemEntity;
pub use pig::PigEntity;


/// Base class for entity.
#[derive(Debug)]
pub struct Base<I> {
    /// The internal entity id.
    pub id: u32,
    /// The current entity position.
    pub pos: DVec3,
    /// The current entity velocity.
    pub vel: DVec3,
    /// Yaw/Pitch look, angles are in radian with no range guarantee.
    pub look: Vec2,
    /// Lifetime of the entity, in ticks.
    pub lifetime: u32,
    /// Is this entity responding to block's collisions.
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
            lifetime: 0,
            no_clip: false,
            on_ground: false,
            in_water: false,
            in_lava: false,
            fall_distance: 0.0,
            fire_ticks: 0,
            health: 1,
            rider_id: None,
            rand: JavaRandom::new_seeded(),
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

        // TODO: Handle water velocity.
        self.in_water = false;

        if self.in_water {
            self.fire_ticks = 0;
            self.fall_distance = 0.0;
        }

        if self.fire_ticks != 0 {
            if false { // if fire immune
                self.fire_ticks = self.fire_ticks.saturating_sub(4);
            } else {
                if self.fire_ticks % 20 == 0 {
                    // TODO: Damage entity
                }
                self.fire_ticks -= 1;
            }
        }

        // TODO: Is entity in lava.
        self.in_lava = false;

        if self.pos.y < -64.0 {
            world.kill_entity(self.id);
        }

    }

    /// Common method for moving an entity by a given amount while checking collisions.
    /// 
    /// **This is the only method to use for actually modifying the entity position.**
    pub fn move_entity(&mut self, world: &mut World, delta: DVec3, step_height: f32) {

        let prev_pos = self.pos;

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

            // println!("== Moving from {}", self.pos);
            // println!(" = Expanded bb: {:?}", bb.expand(delta));
            // println!(" = Colliding bbs ({}): {:?}", colliding_bbs.len(), &colliding_bbs[..]);

            // Compute a new delta that doesn't collide with above boxes.
            let mut new_delta = delta;

            // Check collision on Y axis.
            for colliding_bb in &colliding_bbs {
                new_delta.y = colliding_bb.calc_y_delta(bb, new_delta.y);
            }

            bb += DVec3::new(0.0, new_delta.y, 0.0);

            // Check collision on X axis.
            for colliding_bb in &colliding_bbs {
                new_delta.x = colliding_bb.calc_x_delta(bb, new_delta.x);
            }

            bb += DVec3::new(new_delta.x, 0.0, 0.0);

            // Check collision on Z axis.
            for colliding_bb in &colliding_bbs {
                new_delta.z = colliding_bb.calc_z_delta(bb, new_delta.z);
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

        let pos_delta = self.pos - prev_pos;
        if pos_delta != DVec3::ZERO {
            world.push_event(Event::EntityMoveAndLook { 
                id: self.id, 
                pos_delta,
                look: self.look,
            })
        }

    }

    /// Common tick function to apply the given gravity on the entity and move it, while
    /// managing block collisions.
    pub fn apply_gravity(&mut self, world: &mut World, step_height: f32) {
        self.vel.y -= 0.04;
        self.move_entity(world, self.vel, step_height);
        self.vel *= 0.98;
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
                    // TODO: block::fluid::calc_fluid_velocity(world, pos)
                }

            }
        }

        todo!()

    }

}


/// Base class for living entity.
#[derive(Debug, Default)]
pub struct Living<I> {
    /// The strafing acceleration.
    pub accel_strafing: f32,
    /// The forward acceleration.
    pub accel_forward: f32,
    /// True if this entity is trying to jump.
    pub jumping: bool,
    /// Inner implementation of the living entity.
    pub living: I,
}

impl<I> Base<Living<I>> {

    /// Update living entity AI and jump behaviors.
    pub fn update_living_entity(&mut self, world: &mut World, ai: fn(&mut Self, &mut World)) {

        if self.health == 0 {
            self.base.jumping = false;
            self.base.accel_strafing = 0.0;
            self.base.accel_forward = 0.0;
        } else {
            ai(self, world);
        }

        if self.base.jumping {
            if self.in_water || self.in_lava {
                self.vel.y += 0.04;
            } else if self.base.jumping {
                self.vel.y += 0.42;
            }
        }

        self.base.accel_strafing *= 0.98;
        self.base.accel_forward *= 0.98;

    }

    /// Default AI function for living entities.
    pub fn update_living_ai(&mut self, world: &mut World) {
        
    }

    /// Accelerate a living entity with the given strafing and forward accelerations.
    pub fn accel_living_entity(&mut self, factor: f32) {
        let mut strafing = self.base.accel_strafing;
        let mut forward = self.base.accel_forward;
        let mut dist = Vec2::new(forward, strafing).length();
        if dist >= 0.01 {
            dist = dist.min(1.0);
            dist = factor / dist;
            strafing *= dist;
            forward *= dist;
            let (yaw_sin, yaw_cos) = self.look.x.sin_cos();
            self.vel.x += (strafing * yaw_cos - forward * yaw_sin) as f64;
            self.vel.z += (forward * yaw_cos - strafing * yaw_sin) as f64;
        }
    }

    /// Move a living entity from its forward and strafing accelerations.
    pub fn move_living_entity(&mut self, world: &mut World, step_height: f32) {

        if self.in_water {
            self.accel_living_entity(0.02);
            self.move_entity(world, self.vel, step_height);
            self.vel *= 0.8;
            self.vel.y -= 0.02;
            // TODO: If collided horizontally
        } else if self.in_lava {
            self.accel_living_entity(0.02);
            self.move_entity(world, self.vel, step_height);
            self.vel *= 0.5;
            self.vel.y -= 0.02;
            // TODO: If collided horizontally
        } else {

            let mut factor = 0.91;

            if self.on_ground {
                factor = 546.0 * 0.1 * 0.1 * 0.1;
                let ground_pos = self.pos.as_ivec3();
                if let Some((block, _)) = world.block_and_metadata(ground_pos) {
                    if block != 0 {
                        factor = block_from_id(block).slipperiness * 0.91;
                    }
                }
            }

            self.accel_living_entity(match self.on_ground {
                true => 0.1 * 0.16277136 / (factor * factor * factor),
                false => 0.02,
            });
            
            // TODO: Is on ladder

            self.move_entity(world, self.vel, step_height);

            // TODO: Collided horizontally and on ladder

            self.vel.y -= 0.08;
            self.vel *= DVec3::new(factor as f64, 0.98, factor as f64);

        }

        // TODO: Remaining?

    }

}


/// Base class for living entity.
#[derive(Debug, Default)]
pub struct Creature<I> {
    /// The path this creature needs to follow.
    pub path: Option<Path>,
    /// Inner implementation of the creature entity.
    pub creature: I,
}

impl<I> Base<Living<Creature<I>>> {
    
    /// Update a standard AI.
    pub fn update_creature_ai(&mut self, world: &mut World) {

        // TODO: Work on mob AI with attacks...

        if self.base.living.path.is_none() || self.rand.next_int_bounded(20) != 0 {
            if self.rand.next_int_bounded(80) == 0 {
                // TODO: Find new path.
            }
        }

        if let Some(path) = &mut self.base.living.path {
            if self.rand.next_int_bounded(100) != 0 {

                let bb_size = self.bounding_box.size();
                let double_width = bb_size.x * 2.0;

                let mut next_pos = None;
                
                while let Some(point) = path.point() {

                    let mut pos = point.pos.as_dvec3();
                    pos.x += bb_size.x * 0.5;
                    pos.z += bb_size.z * 0.5;

                    // Advance the path to the next point only if distance to current
                    // one is too short.
                    let pos_dist_sq = pos.distance_squared(DVec3::new(self.pos.x, pos.y, self.pos.z));
                    if pos_dist_sq < double_width * double_width {
                        path.advance();
                    } else {
                        next_pos = Some(pos);
                        break;
                    }

                }

                self.base.jumping = false;

                if let Some(next_pos) = next_pos {

                    let dx = next_pos.x - self.pos.x;
                    let dy = self.bounding_box.min.y.add(0.5).floor();
                    let dz = next_pos.z - self.pos.z;

                    let target_yaw = f64::atan2(dx, dz) as f32 - std::f32::consts::FRAC_PI_2;
                    let delta_yaw = target_yaw - self.look.x;

                    self.look.x += delta_yaw;

                    if dy > 0.0 {
                        self.base.jumping = true;
                    }

                }

                // TODO: If player to attack

                // TODO: If collided horizontal and no path, then jump

                if self.rand.next_float() < 0.8 && (self.in_water || self.in_water) {
                    self.base.jumping = true;
                }

                return;

            }
        }

        self.update_living_ai(world);
        self.base.living.path = None;

    }

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


#[derive(Debug)]
pub struct Path {
    points: Vec<PathPoint>,
    index: usize,
}

#[derive(Debug)]
pub struct PathPoint {
    pub pos: IVec3,
    pub distance_to_next: f32,
    pub distance_to_target: f32,
}

impl Path {

    /// Return the current path position.
    pub fn point(&self) -> Option<&PathPoint> {
        self.points.get(self.index)
    }

    pub fn advance(&mut self) {
        self.index += 1;
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