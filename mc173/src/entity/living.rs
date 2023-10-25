//! Living entity logic implementation.

use std::ops::Add;

use glam::{Vec2, IVec3, DVec3};

use crate::block;
use crate::path::PathFinder;
use crate::world::World;

use super::{Base, Living, Size, Path};


impl<I> Base<Living<I>> {

    /// Tick a living entity.
    pub fn tick_living<F>(&mut self, world: &mut World, size: Size, ia: F)
    where
        F: FnOnce(&mut Self, &mut World),
    {

        self.tick_base(world, size);

        ia(self, world);

        if self.kind.jumping {
            if self.in_water || self.in_lava {
                self.vel_dirty = true;
                self.vel.y += 0.04;
            } else if self.kind.jumping {
                self.vel_dirty = true;
                self.vel.y += 0.42;
            }
        }

        self.kind.accel_strafing *= 0.98;
        self.kind.accel_forward *= 0.98;
        self.kind.yaw_velocity *= 0.9;

    }

    /// Update the entity velocity depending on its current strafing and forward 
    /// accelerations toward its yaw angle.
    fn update_living_vel(&mut self, factor: f32) {
        let mut strafing = self.kind.accel_strafing;
        let mut forward = self.kind.accel_forward;
        let mut dist = Vec2::new(forward, strafing).length();
        if dist >= 0.01 {
            dist = dist.min(1.0);
            dist = factor / dist;
            strafing *= dist;
            forward *= dist;
            let (yaw_sin, yaw_cos) = self.look.x.sin_cos();
            self.vel_dirty = true;
            self.vel.x += (strafing * yaw_cos - forward * yaw_sin) as f64;
            self.vel.z += (forward * yaw_cos + strafing * yaw_sin) as f64;
        }
    }

    /// Move a living entity from its forward and strafing accelerations.
    pub fn update_living_pos(&mut self, world: &mut World, step_height: f32) {

        if self.in_water {
            self.update_living_vel(0.02);
            self.update_pos_move(world, self.vel, step_height);
            self.vel *= 0.8;
            self.vel.y -= 0.02;
            // TODO: If collided horizontally
        } else if self.in_lava {
            self.update_living_vel(0.02);
            self.update_pos_move(world, self.vel, step_height);
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

            // Change entity velocity if on ground or not.
            let vel_factor = match self.on_ground {
                true => 0.1 * 0.16277136 / (slipperiness * slipperiness * slipperiness),
                false => 0.02,
            };

            self.update_living_vel(vel_factor);
            
            // TODO: Is on ladder

            self.update_pos_move(world, self.vel, step_height);

            // TODO: Collided horizontally and on ladder

            self.vel.y -= 0.08;
            self.vel.x *= slipperiness as f64;
            self.vel.y *= 0.98;
            self.vel.z *= slipperiness as f64;

        }
        
        self.vel_dirty = true;

        // TODO: Remaining?

    }

    /// Default AI function for living entities.
    pub fn update_ai(&mut self, world: &mut World) {
        
        // TODO: Handle kill when closest player is too far away.

        self.kind.accel_strafing = 0.0;
        self.kind.accel_forward = 0.0;

        // Maximum of 8 block to look at.
        let look_target_range_squared = 8.0 * 8.0;

        if self.rand.next_float() < 0.02 {
            // TODO: Look at closest player (max 8 blocks).
        }

        // If the entity should have a target, just look at it if possible, and stop if
        // the target should end or is too far away.
        if let Some(target) = &mut self.kind.look_target {

            target.ticks_remaining -= 1;
            let mut target_release = target.ticks_remaining == 0;

            if let Some(target_entity) = world.entity(target.entity_id) {
                // TODO: Fix the Y value, in order to look at eye height.
                let target_pos = target_entity.base().pos;
                // TODO: Pitch step should be an argument, 40 by default, but 20 for 
                // sitting dogs.
                self.update_look_at_by_step(target_pos, Vec2::new(10f32.to_radians(), 40f32.to_radians()));
                // Indicate if the entity is still in range.
                if target_pos.distance_squared(self.pos) > look_target_range_squared {
                    target_release = false;
                }
            } else {
                // Entity is dead.
                target_release = false;
            }

            if target_release {
                self.kind.look_target = None;
            }

        } else {

            if self.rand.next_float() < 0.05 {
                self.kind.yaw_velocity = (self.rand.next_float() - 0.5) * 20f32.to_radians();
            }

            self.look.x += self.kind.yaw_velocity;
            self.look.y = 0.0;

        }

        if self.in_water || self.in_lava {
            self.kind.jumping = self.rand.next_float() < 0.8;
        }

    }

    /// Default AI function for "creatures".
    pub fn update_creature_ai<W>(&mut self, 
        world: &mut World, 
        move_speed: f32, 
        weight_func: W)
    where
        W: Fn(&World, IVec3) -> f32,
    {

        // TODO: Work on mob AI with attacks...

        if self.kind.path.is_none() || self.rand.next_int_bounded(20) != 0 {
            // Find a new path every 4 seconds on average.
            if self.rand.next_int_bounded(80) == 0 {
                self.update_creature_path(world, weight_func);
            }
        }

        if let Some(path) = &mut self.kind.path {
            if self.data.rand.next_int_bounded(100) != 0 {

                let bb_size = self.data.bb.size();
                let double_width = bb_size.x * 2.0;

                let mut next_pos = None;
                
                while let Some(pos) = path.point() {

                    let mut pos = pos.as_dvec3();
                    pos.x += (bb_size.x + 1.0) * 0.5;
                    pos.z += (bb_size.z + 1.0) * 0.5;

                    // Advance the path to the next point only if distance to current
                    // one is too short.
                    let pos_dist_sq = pos.distance_squared(DVec3::new(self.data.pos.x, pos.y, self.data.pos.z));
                    if pos_dist_sq < double_width * double_width {
                        path.advance();
                    } else {
                        next_pos = Some(pos);
                        break;
                    }

                }

                self.kind.jumping = false;

                if let Some(next_pos) = next_pos {

                    // println!("== update_creature_ai: next pos {next_pos}");

                    let dx = next_pos.x - self.pos.x;
                    let dy = next_pos.y - self.bb.min.y.add(0.5).floor();
                    let dz = next_pos.z - self.pos.z;

                    let target_yaw = f64::atan2(dx, dz) as f32 - std::f32::consts::FRAC_PI_2;
                    let delta_yaw = target_yaw - self.look.x;

                    self.kind.accel_forward = move_speed;
                    self.look.x += delta_yaw;

                    if dy > 0.0 {
                        self.kind.jumping = true;
                    }

                } else {
                    // println!("== update_creature_ai: finished path");
                    self.kind.path = None;
                }

                // TODO: If player to attack

                // TODO: If collided horizontal and no path, then jump

                if self.rand.next_float() < 0.8 && (self.in_water || self.in_water) {
                    self.kind.jumping = true;
                }

                return;

            } else {
                // println!("== update_creature_ai: bad luck, path abandoned");
            }
        }

        // println!("== update_creature_ai: no path, fallback to living ai");

        // If we can't run a path finding AI, fallback to the default immobile AI.
        self.kind.path = None;
        self.update_ai(world);

    }

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

    /// Find a new path to go to for this creature entity.
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

            if let Some(points) = path_finder.find_path_from_bounding_box(self.bb, best_pos, 18.0) {
                // println!("== update_creature_path: new path found to {best_pos}");
                self.kind.path = Some(Path {
                    points,
                    index: 0,
                })
            }

        }

    }

}