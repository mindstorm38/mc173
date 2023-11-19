//! Dispenser block entity.

use crate::inventory::Inventory;


#[derive(Debug, Clone)]
pub struct DispenserBlockEntity {
    /// The inventory of the dispenser.
    pub inv: Inventory,
}

impl Default for DispenserBlockEntity {
    fn default() -> Self {
        Self {
            inv: Inventory::new(9),
        }
    }
}
