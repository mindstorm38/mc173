//! Item entity logic implementation.

use glam::IVec3;

use crate::world::World;
use crate::util::Face;
use crate::block;

use super::{ItemEntity, Size};


impl ItemEntity {

    /// Tick the item entity.
    pub fn tick_item(&mut self, world: &mut World, id: u32) {

        self.tick_base(world, id, Size::new_centered(0.25, 0.25));
        
        if self.kind.frozen_ticks > 0 {
            self.kind.frozen_ticks -= 1;
        }
    
        // Update item velocity.
        self.vel_dirty = true;
        self.vel.y -= 0.04;
    
        // If the item is in lava, apply random motion like it's burning.
        // NOTE: The real client don't use 'in_lava', check if problematic.
        if self.in_lava {
            self.vel.y = 0.2;
            self.vel.x = ((self.rand.next_float() - self.rand.next_float()) * 0.2) as f64;
            self.vel.z = ((self.rand.next_float() - self.rand.next_float()) * 0.2) as f64;
        }

        // If the item is in an opaque block.
        let block_pos = self.pos.floor().as_ivec3();
        if world.is_block_opaque_cube(block_pos) {

            let delta = self.pos - block_pos.as_dvec3();

            // Find a block face where we can bump the item.
            let bump_face = Face::ALL.into_iter()
                .filter(|face| !world.is_block_opaque_cube(block_pos + face.delta()))
                .map(|face| {
                    let mut delta = delta[face.axis_index()];
                    if face.is_pos() {
                        delta = 1.0 - delta;
                    }
                    (face, delta)
                })
                .min_by(|&(_, delta1), &(_, delta2)| delta1.total_cmp(&delta2))
                .map(|(face, _)| face);

            // If we found a non opaque face then we bump the item to that face.
            if let Some(bump_face) = bump_face {
                let accel = (self.rand.next_float() * 0.2 + 0.1) as f64;
                if bump_face.is_neg() {
                    self.vel[bump_face.axis_index()] = -accel;
                } else {
                    self.vel[bump_face.axis_index()] = accel;
                }
            }
            
        }
    
        // TODO: Item motion if stuck in a block.
    
        // Move the item while checking collisions if needed.
        self.update_pos_move(world, self.vel, 0.0);
    
        let mut slipperiness = 0.98;
    
        if self.on_ground {
    
            slipperiness = 0.1 * 0.1 * 58.8;
    
            let ground_pos = IVec3 {
                x: self.pos.x.floor() as i32,
                y: self.bb.min.y.floor() as i32 - 1,
                z: self.pos.z.floor() as i32,
            };
    
            if let Some((ground_id, _)) = world.get_block(ground_pos) {
                if ground_id != block::AIR {
                    slipperiness = block::material::get_slipperiness(ground_id);
                }
            }
    
        }
    
        // Slow its velocity depending on ground slipperiness.
        self.vel.x *= slipperiness as f64;
        self.vel.y *= 0.98;
        self.vel.z *= slipperiness as f64;
        
        if self.on_ground {
            self.vel.y *= -0.5;
        }
    
        // Kill the item self after 5 minutes (5 * 60 * 20).
        if self.lifetime >= 6000 {
            world.remove_entity(id);
        }

    }

}
