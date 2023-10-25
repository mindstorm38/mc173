//! Base entity logic implementation.

use std::ops::Sub;

use glam::{DVec3, Vec2};

use crate::block::{self, Material};
use crate::util::BoundingBox;
use crate::world::World;

use super::{Base, Size, Entity};


impl<I> Base<I> {

    /// Base function for updating the entity.
    pub fn tick_base(&mut self, world: &mut World, size: Size) {

        // If size is none, update it and the bounding box.
        if !self.coherent {
            self.size = size;
            self.update_bounding_box_from_pos();
        }

        // Just kill the entity if far in the void.
        if self.pos.y < -64.0 {
            world.kill_entity(self.id);
            return; // TODO: Early return in caller function.
        }

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

        // Check if there is a lava block colliding...
        let lava_bb = self.bb.inflate(DVec3::new(-0.1, -0.4, -0.1));
        self.in_lava = world.iter_blocks_in_box(lava_bb)
            .any(|(_, block, _)| block::from_id(block).material == Material::Lava);


    }

    /// Common method for moving an entity by a given amount while checking collisions.
    pub fn update_pos_move(&mut self, world: &mut World, delta: DVec3, step_height: f32) {

        if self.no_clip {
            self.bb += delta;
        } else {

            // TODO: 

            // TODO: If in cobweb:
            // delta *= DVec3::new(0.25, 0.05, 0.25)
            // base.vel = DVec3::ZERO

            // TODO: Sneaking on ground

            let colliding_bb = self.bb.expand(delta);
            let colliding_bbs: Vec<_> = world.iter_blocks_boxes_colliding(colliding_bb)
                .chain(world.iter_entities_colliding(colliding_bb)
                    .filter_map(|(entity, entity_bb)| {
                        // Only the boat entity acts like a hard bounding box.
                        if let Entity::Boat(_) = entity {
                            Some(entity_bb)
                        } else {
                            None
                        }
                    }))
                .collect();
            
            // Compute a new delta that doesn't collide with above boxes.
            let mut new_delta = delta;

            // Check collision on Y axis.
            for colliding_bb in &colliding_bbs {
                new_delta.y = colliding_bb.calc_y_delta(self.bb, new_delta.y);
            }

            self.bb += DVec3::new(0.0, new_delta.y, 0.0);

            // Check collision on X axis.
            for colliding_bb in &colliding_bbs {
                new_delta.x = colliding_bb.calc_x_delta(self.bb, new_delta.x);
            }

            self.bb += DVec3::new(new_delta.x, 0.0, 0.0);

            // Check collision on Z axis.
            for colliding_bb in &colliding_bbs {
                new_delta.z = colliding_bb.calc_z_delta(self.bb, new_delta.z);
            }
            
            self.bb += DVec3::new(0.0, 0.0, new_delta.z);

            let collided_x = delta.x != new_delta.x;
            let collided_y = delta.y != new_delta.y;
            let collided_z = delta.z != new_delta.z;
            let on_ground = collided_y && delta.y < 0.0; // || self.on_ground

            // Apply step if relevant.
            if step_height > 0.0 && on_ground && (collided_x || collided_z) {
                // TODO: todo!("handle step motion");
            }

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

        self.update_pos_from_bounding_box();

    }
    
    /// This function recompute the current bounding box from the position and the last
    /// size that was used to create it.
    pub fn update_bounding_box_from_pos(&mut self) {
        let half_width = (self.size.width / 2.0) as f64;
        let height = self.size.height as f64;
        let height_center = self.size.height_center as f64;
        self.bb = BoundingBox {
            min: self.pos - DVec3::new(half_width, height_center, half_width),
            max: self.pos + DVec3::new(half_width, height + height_center, half_width),
        };
        // Entity position and bounding are coherent.
        self.coherent = true;
    }

    /// This position recompute the current position based on the bounding box' position
    /// the size that was used to create it.
    pub fn update_pos_from_bounding_box(&mut self) {
        
        let height_center = self.size.height_center as f64;
        let new_pos = DVec3 {
            x: (self.bb.min.x + self.bb.max.x) / 2.0,
            y: self.bb.min.y + height_center,
            z: (self.bb.min.z + self.bb.max.z) / 2.0,
        };

        if new_pos != self.pos {
            self.pos = new_pos;
            self.pos_dirty = true;
        }
        
    }

    /// Modify the look angles of this entity, limited to the given step. We you need to
    /// call this function many time to reach the desired look.
    pub fn update_look_by_step(&mut self, look: Vec2, step: Vec2) {
        let look = look.rem_euclid(Vec2::splat(std::f32::consts::TAU));
        let delta = look.sub(self.look).min(step);
        if delta != Vec2::ZERO {
            self.look_dirty = true;
            self.look += delta;
        }
    }

    /// Modify the look angles to point to a given target step by step.
    pub fn update_look_at_by_step(&mut self, target: DVec3, step: Vec2) {
        let delta = target - self.pos;
        let horizontal_dist = delta.length();
        let yaw = f64::atan2(delta.z, delta.x) as f32 - std::f32::consts::FRAC_PI_2;
        let pitch = -f64::atan2(delta.y, horizontal_dist) as f32;
        self.update_look_by_step(Vec2::new(yaw, pitch), step);
    }

}
