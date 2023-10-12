//! Item entity implementation.

use crate::item::ItemStack;
use crate::world::World;

use super::{EntityBehavior, Base, Size};


#[derive(Debug, Default)]
pub struct Item {
    /// The item stack represented by this entity.
    pub item_stack: ItemStack,
    /// Tick count before this item entity can be picked up.
    pub time_before_pickup: u32,
}

/// A falling block entity.
pub type ItemEntity = Base<Item>;

impl EntityBehavior for ItemEntity {

    fn tick(&mut self, world: &mut World) {
        
        // self.apply_gravity(world, Size::new_centered(0.25, 0.25));

    }

}
