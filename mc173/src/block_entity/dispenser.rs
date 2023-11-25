//! Dispenser block entity.

use crate::item::ItemStack;


#[derive(Debug, Clone, Default)]
pub struct DispenserBlockEntity {
    /// The inventory of the dispenser.
    pub inv: Box<[ItemStack; 9]>,
}
