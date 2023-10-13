//! Item entity implementation.

use crate::item::ItemStack;
use crate::world::World;

use super::{EntityLogic, Base, Size};


#[derive(Debug, Default)]
pub struct Item {
    /// The item stack represented by this entity.
    pub item: ItemStack,
    /// Tick count before this item entity can be picked up.
    pub time_before_pickup: u32,
}

/// A falling block entity.
pub type ItemEntity = Base<Item>;

impl EntityLogic for ItemEntity {

    fn tick(&mut self, world: &mut World) {
        
        self.update_entity(world, Size::new_centered(0.25, 0.25));

    }

}
