//! Entity data structures, no logic is defined here.

use std::ops::{Add, Sub};
use std::any::Any;

use glam::{DVec3, Vec2, IVec3};

use crate::block::{self, Material};
use crate::path::PathFinder;
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
    /// with [`update_bounding_box`] or [`update`] methods.
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

    /// Common method to update entities..
    pub fn update(&mut self, world: &mut World) {

        self.lifetime += 1;

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

    /// Update the internal bounding box depending on the entity position and given 
    /// bounding box size. Usually called through the [`update`] method.
    pub fn update_bounding_box(&mut self, size: Size) {
        let half_width = (size.width / 2.0) as f64;
        let height = size.height as f64;
        let height_center = size.height_center as f64;
        self.bounding_box = BoundingBox {
            min: self.pos - DVec3::new(half_width, height_center, half_width),
            max: self.pos + DVec3::new(half_width, height + height_center, half_width),
        };
    }

    /// Common method for moving an entity by a given amount while checking collisions.
    pub fn update_position_delta(&mut self, world: &mut World, delta: DVec3, step_height: f32) {

        if self.no_clip {
            self.bounding_box += delta;
            self.update_position(world, self.pos + delta);
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
            self.bounding_box = bb;

            let collided_x = delta.x != new_delta.x;
            let collided_y = delta.y != new_delta.y;
            let collided_z = delta.z != new_delta.z;
            let on_ground = collided_y && delta.y < 0.0; // || self.on_ground

            // Apply step if relevant.
            if step_height > 0.0 && on_ground && (collided_x || collided_z) {
                // TODO: todo!("handle step motion");
            }

            self.update_position(world, self.pos + new_delta);
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

    /// Update position of the entity, sending an event if needed.
    /// 
    /// **This is the only method to use for actually modifying the entity position.**
    pub fn update_position(&mut self, world: &mut World, pos: DVec3) {
        if pos != self.pos {
            self.pos = pos;
            world.push_event(Event::EntityPosition { 
                id: self.id, 
                pos,
            })
        }
    }

    /// Update look of the entity, sending an event if needed.
    /// 
    /// **This is the only method to use for actually modifying the entity look.**
    pub fn update_look(&mut self, world: &mut World, look: Vec2) {

        if look != self.look {
            self.look = look;
            self.look = self.look.rem_euclid(Vec2::splat(std::f32::consts::TAU));
            world.push_event(Event::EntityLook { 
                id: self.id, 
                look,
            })
        }

    }

    pub fn calc_eye_height(&self) -> f32 {
        self.bounding_box.size().y as f32 * 0.85
    }

    pub fn calc_fluid_velocity(&mut self, world: &mut World, material: Material) -> DVec3 {

        let fluid_bb = self.bounding_box.inflate(DVec3::new(-0.001, -0.4 - 0.001, -0.001));
        let min = fluid_bb.min.floor().as_ivec3();
        let max = fluid_bb.max.add(1.0).floor().as_ivec3();

        for (pos, block, metadata) in world.iter_area_blocks(min, max) {
            if block::from_id(block).material == material {

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
    /// Velocity of the look's yaw axis.
    pub yaw_velocity: f32,
    /// True if this entity is trying to jump.
    pub jumping: bool,
    /// If this entity is looking at another one.
    pub look_target: Option<LookTarget>,
    /// Inner implementation of the living entity.
    pub living: I,
}

/// Define a target for an entity to look at.
#[derive(Debug, Default)]
pub struct LookTarget {
    /// The entity id to look at.
    pub entity_id: u32,
    /// Ticks remaining before stop looking at it.
    pub ticks_remaining: u32,
}

impl<I> Base<Living<I>> {

    /// Update living entity AI and jump behaviors
    pub fn update_living<F>(&mut self, world: &mut World, ai_func: F)
    where
        F: FnOnce(&mut Self, &mut World),
    {

        if self.health == 0 {
            self.base.jumping = false;
            self.base.accel_strafing = 0.0;
            self.base.accel_forward = 0.0;
        } else {
            ai_func(self, world);
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
        self.base.yaw_velocity *= 0.9;

    }

    /// Default AI function for living entities.
    pub fn update_living_ai(&mut self, world: &mut World) {
        
        // TODO: Handle kill when closest player is too far away.

        self.base.accel_strafing = 0.0;
        self.base.accel_forward = 0.0;

        // Maximum of 8 block to look at.
        let look_target_range = 8.0;

        if self.rand.next_float() < 0.02 {
            // TODO: Look at closest player (max 8 blocks).
        }

        if let Some(target) = &mut self.base.look_target {

            target.ticks_remaining -= 1;
            let mut target_release = target.ticks_remaining == 0;

            if let Some(target_entity) = world.entity(target.entity_id) {
                // FIXME: Fix the Y value, in order to look at eye height.
                // FIXME: Pitch step should be an argument.
                let target_pos = target_entity.pos();
                self.update_living_look_at(world, target_pos, Vec2::new(10.0, 10.0));
                // Indicate if the entity is still in range.
                if target_pos.distance_squared(self.pos) > look_target_range * look_target_range {
                    target_release = false;
                }
            } else {
                // Entity is dead.
                target_release = false;
            }

            if target_release {
                self.base.look_target = None;
            }

        } else {

            if self.rand.next_float() < 0.05 {
                self.base.yaw_velocity = (self.rand.next_float() - 0.5) * 20f32.to_radians();
            }

            self.update_look(world, Vec2::new(self.look.x + self.base.yaw_velocity, 0.0));

        }

        if self.in_water || self.in_lava {
            self.base.jumping = self.rand.next_float() < 0.8;
        }

    }

    /// Modify a living look step by step.
    pub fn update_living_look(&mut self, world: &mut World, look: Vec2, step: Vec2) {
        let look = look.rem_euclid(Vec2::splat(std::f32::consts::TAU));
        let delta = look.sub(self.look).min(step);
        self.update_look(world, self.look + delta);
    }

    /// Make this living entity face a parti
    pub fn update_living_look_at(&mut self, world: &mut World, target: DVec3, step: Vec2) {
        let delta = target - self.pos;
        let horizontal_dist = delta.length();
        let yaw = f64::atan2(delta.z, delta.x) as f32 - std::f32::consts::FRAC_PI_2;
        let pitch = -f64::atan2(delta.y, horizontal_dist) as f32;
        self.update_living_look(world, Vec2::new(yaw, pitch), step);
    }

    /// Accelerate a living entity with the given strafing and forward accelerations.
    pub fn accel_living(&mut self, factor: f32) {
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
            self.vel.z += (forward * yaw_cos + strafing * yaw_sin) as f64;
        }
    }

    /// Move a living entity from its forward and strafing accelerations.
    pub fn update_living_position(&mut self, world: &mut World, step_height: f32) {

        if self.in_water {
            self.accel_living(0.02);
            self.update_position_delta(world, self.vel, step_height);
            self.vel *= 0.8;
            self.vel.y -= 0.02;
            // TODO: If collided horizontally
        } else if self.in_lava {
            self.accel_living(0.02);
            self.update_position_delta(world, self.vel, step_height);
            self.vel *= 0.5;
            self.vel.y -= 0.02;
            // TODO: If collided horizontally
        } else {

            let mut slipperiness = 0.91;

            if self.on_ground {
                slipperiness = 546.0 * 0.1 * 0.1 * 0.1;
                let ground_pos = self.pos.as_ivec3();
                if let Some((block, _)) = world.block_and_metadata(ground_pos) {
                    if block != 0 {
                        slipperiness = block::from_id(block).slipperiness * 0.91;
                    }
                }
            }

            self.accel_living(match self.on_ground {
                true => 0.1 * 0.16277136 / (slipperiness * slipperiness * slipperiness),
                false => 0.02,
            });
            
            // TODO: Is on ladder

            self.update_position_delta(world, self.vel, step_height);

            // TODO: Collided horizontally and on ladder

            self.vel.y -= 0.08;
            self.vel *= DVec3::new(slipperiness as f64, 0.98, slipperiness as f64);

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
    
    /// Specialization of [`update_creature_ai`] for basic animals, the weight function
    /// privileges grass, and then light level.
    pub fn update_animal_ai(&mut self, world: &mut World) {
        self.update_creature_ai(world, 0.7, |world, pos| {
            if let Some((block::GRASS, _)) = world.block_and_metadata(pos - IVec3::Y) {
                10.0
            } else {
                // TODO: Get light at position and subtract 0.5 to light level
                0.0
            }
        })
    }

    /// Update a standard AI.
    pub fn update_creature_ai<W>(&mut self, world: &mut World, move_speed: f32, weight_func: W)
    where
        W: Fn(&World, IVec3) -> f32,
    {

        // TODO: Work on mob AI with attacks...

        if self.base.living.path.is_none() || self.rand.next_int_bounded(20) != 0 {
            // Find a new path every 4 seconds on average.
            if self.rand.next_int_bounded(80) == 0 {
                self.update_creature_path(world, weight_func);
            }
        }

        if let Some(path) = &mut self.base.living.path {
            if self.rand.next_int_bounded(100) != 0 {

                let bb_size = self.bounding_box.size();
                let double_width = bb_size.x * 2.0;

                let mut next_pos = None;
                
                while let Some(pos) = path.point() {

                    let mut pos = pos.as_dvec3();
                    pos.x += (bb_size.x + 1.0) * 0.5;
                    pos.z += (bb_size.z + 1.0) * 0.5;

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

                    // println!("== update_creature_ai: next pos {next_pos}");

                    let dx = next_pos.x - self.pos.x;
                    let dy = next_pos.y - self.bounding_box.min.y.add(0.5).floor();
                    let dz = next_pos.z - self.pos.z;

                    let target_yaw = f64::atan2(dx, dz) as f32 - std::f32::consts::FRAC_PI_2;
                    let delta_yaw = target_yaw - self.look.x;

                    self.base.accel_forward = move_speed;
                    self.update_look(world, self.look + Vec2::X * delta_yaw);

                    if dy > 0.0 {
                        self.base.jumping = true;
                    }

                } else {
                    // println!("== update_creature_ai: finished path");
                    self.base.living.path = None;
                }

                // TODO: If player to attack

                // TODO: If collided horizontal and no path, then jump

                if self.rand.next_float() < 0.8 && (self.in_water || self.in_water) {
                    self.base.jumping = true;
                }

                return;

            } else {
                // println!("== update_creature_ai: bad luck, path abandoned");
            }
        }

        // println!("== update_creature_ai: no path, fallback to living ai");

        self.update_living_ai(world);
        self.base.living.path = None;

    }

    /// Find a path for a create entity.
    pub fn update_creature_path<W>(&mut self, world: &World, weight_func: W)
    where
        W: Fn(&World, IVec3) -> f32,
    {

        // println!("== update_creature_path: entry");

        let mut best_pos = None;

        for _ in 0..10 {

            let try_pos = IVec3 {
                x: self.pos.x.add((self.rand.next_int_bounded(13) - 6) as f64).floor() as i32,
                y: self.pos.y.add((self.rand.next_int_bounded(7) - 3) as f64).floor() as i32,
                z: self.pos.z.add((self.rand.next_int_bounded(13) - 6) as f64).floor() as i32,
            };
            
            let try_weight = weight_func(world, try_pos);
            if let Some((_, weight)) = best_pos {
                if try_weight > weight {
                    best_pos = Some((try_pos, try_weight));
                }
            } else {
                best_pos = Some((try_pos, try_weight));
            }

        }

        if let Some((best_pos, _weight)) = best_pos {

            let mut path_finder = PathFinder::new(world);
            let best_pos = best_pos.as_dvec3() + 0.5;

            if let Some(points) = path_finder.find_path_from_bounding_box(self.bounding_box, best_pos, 18.0) {
                // println!("== update_creature_path: new path found to {best_pos}");
                self.base.living.path = Some(Path {
                    points,
                    index: 0,
                })
            }

        }

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

    pub fn advance(&mut self) {
        self.index += 1;
    }
    
}


/// This trait can be used to implement entity logic. It also requires your type to also
/// implement the [`Any`] trait, this provides downcasts on dynamic pointers to entities.
pub trait EntityLogic: Any {

    /// Get the size of the entity in its current state. This is used at each tick to 
    /// update the bounding box of all entities before actually ticking them. This allows
    /// performing bounding box collisions with all previous and future entities when
    /// actually ticking.
    fn size(&mut self) -> Size;

    /// Tick this entity and update its internal components.
    fn tick(&mut self, world: &mut World);

}


/// Base trait for [`EntityLogic`] implementors, it is automatically implemented for all
/// generic type [`Base`] and provides common methods to access base properties of an
/// entity behind a dynamic reference.
pub trait EntityGeneric: Any {

    /// Get the entity id.
    fn id(&self) -> u32;

    /// Get the entity position.
    fn pos(&self) -> DVec3;

    /// Get the entity look.
    fn look(&self) -> Vec2;

    /// Get this entity as any type, this allows checking its real type.
    fn any(&self) -> &dyn Any;

    /// Get this entity as mutable any type.
    fn any_mut(&mut self) -> &mut dyn Any;

    /// Debug-purpose underlying type name.
    fn type_name(&self) -> &'static str;

    /// Update the internal bounding box of the entity depending on its size.
    fn update_bounding_box(&mut self);

    /// Actually tick the entity, delegating to underlying logic.
    fn tick(&mut self, world: &mut World);

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
    fn look(&self) -> Vec2 {
        self.look
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

    #[inline]
    fn update_bounding_box(&mut self) {
        let size = EntityLogic::size(self);
        self.update_bounding_box(size);
    }

    #[inline]
    fn tick(&mut self, world: &mut World) {
        EntityLogic::tick(self, world);
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