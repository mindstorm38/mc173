//! Item entity implementation.

use glam::{IVec3, DVec3};

use crate::item::ItemStack;
use crate::world::World;
use crate::block;

use super::{EntityLogic, Base, Size};


#[derive(Debug, Default)]
pub struct Item {
    /// The item stack represented by this entity.
    pub item: ItemStack,
    /// Tick count before this item entity can be picked up.
    pub frozen_ticks: u32,
}

/// A falling block entity.
pub type ItemEntity = Base<Item>;

impl EntityLogic for ItemEntity {

    fn tick(&mut self, world: &mut World) {
        
        self.update(world, Size::new_centered(0.25, 0.25));

        if self.base.frozen_ticks > 0 {
            self.base.frozen_ticks -= 1;
        }

        self.vel.y -= 0.04;

        // TODO: handle lava

        self.update_position_delta(world, self.vel, 0.0);

        let mut slipperiness = 0.98;

        if self.on_ground {
            slipperiness = 0.1 * 0.1 * 58.8;
            let ground_pos = IVec3 {
                x: self.pos.x.floor() as i32,
                y: self.bounding_box.min.y.floor() as i32 - 1,
                z: self.pos.z.floor() as i32,
            };
            if let Some((block, _)) = world.block_and_metadata(ground_pos) {
                if block != block::AIR {
                    slipperiness = block::from_id(block).slipperiness * 0.98;
                }
            }
        }

        self.vel *= DVec3::new(slipperiness as f64, 0.98, slipperiness as f64);
        
        if self.on_ground {
            self.vel.y *= -0.5;
        }

        if self.lifetime >= 6000 {
            world.kill_entity(self.id);
        }

    }

}
