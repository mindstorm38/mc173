//! Chest block entity.

use crate::item::inventory::Inventory;


#[derive(Debug, Clone)]
pub struct ChestBlockEntity {
    pub inventory: Inventory,
}

impl Default for ChestBlockEntity {
    fn default() -> Self {
        Self {
            inventory: Inventory::new(36),
        }
    }
}
