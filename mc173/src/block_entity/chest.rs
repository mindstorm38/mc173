//! Chest block entity.

use crate::inventory::Inventory;


#[derive(Debug, Clone)]
pub struct ChestBlockEntity {
    /// The inventory of the chest.
    pub inv: Inventory,
}

impl Default for ChestBlockEntity {
    fn default() -> Self {
        Self {
            inv: Inventory::new(27),
        }
    }
}
