//! Common NBT serde functions for item slots.

use crate::item::ItemStack;

use super::item_stack_nbt;


#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct SlotItemStackNbt {
    #[serde(rename = "Slot")]
    pub slot: u8,
    #[serde(with = "item_stack_nbt", flatten)]
    pub stack: ItemStack,
}

/// Insert a vector of slots into an inventory while checking correctness of slots.
pub fn insert_slots(slots: Vec<SlotItemStackNbt>, inv: &mut [ItemStack]) {
    for slot in slots {
        if (slot.slot as usize) < inv.len() {
            inv[slot.slot as usize] = slot.stack;
        }
    }
}

/// Make a raw NBT slots vector from an inventory.
pub fn make_slots(inv: &[ItemStack]) -> Vec<SlotItemStackNbt> {
    inv.iter()
        .enumerate()
        .filter(|&(slot, stack)| !stack.is_empty() && slot <= 255)
        .map(|(slot, stack)| SlotItemStackNbt {
            slot: slot as u8,
            stack: *stack,
        })
        .collect()
}
