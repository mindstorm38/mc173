//! Inventory data structure storing item stacks.

use crate::item::ItemStack;
use crate::item;


/// An inventory handle is used to assists item insertion into inventory. It also record
/// stack indices that have changed and therefore allows selective events.
pub struct InventoryHandle<'a> {
    inv: &'a mut [ItemStack],
    changes: u64,
}

impl<'a> InventoryHandle<'a> {

    pub fn new(inv: &'a mut [ItemStack]) -> Self {
        assert!(inv.len() <= 64);
        Self {
            inv,
            changes: 0,
        }
    }

    /// Add an item to the inventory. The returned size if the number of items consumed
    /// from the stack, this may not be equal to the stack size of the inventory is full.
    pub fn add(&mut self, stack: ItemStack) -> u16 {

        // Do nothing if stack size is 0 or the item is air.
        if stack.is_empty() {
            return 0;
        }

        let item = item::from_id(stack.id);
        let mut remaining_size = stack.size;

        // Only accumulate of stack size is greater than 1.
        if item.max_stack_size > 1 {
            // Search a slot where the item is compatible.
            for (index, slot) in self.inv.iter_mut().enumerate() {
                // If the slot is of the same item and has space left in the stack size.
                if slot.id == stack.id && slot.damage == stack.damage && slot.size < item.max_stack_size {
                    let available = item.max_stack_size - slot.size;
                    let to_add = available.min(remaining_size);
                    slot.size += to_add;
                    remaining_size -= to_add;
                    // NOTE: We requires that size must be less than 64, so the index fit
                    // in the 64 bits of changes integer.
                    self.changes |= 1 << index;
                    if remaining_size == 0 {
                        return stack.size;
                    }
                }
            }
        }

        // If we land here, some items are remaining to insert in the empty slots.
        // We can also land here if the item has damage value. We search empty slots.
        for (index, slot) in self.inv.iter_mut().enumerate() {
            if slot.is_empty() {
                // We found an empty slot, insert the whole remaining stack size.
                *slot = stack;
                slot.size = remaining_size;
                self.changes |= 1 << index;
                return stack.size;
            }
        }

        // Here we found no available slot so we return the total added size.
        stack.size - remaining_size
        
    }

    /// Iterate over item changes that happened in this inventory, this also returns the
    /// new item at the changed position.
    pub fn iter_changes(&self) -> impl Iterator<Item = usize> {
        let changes = self.changes;
        (0..64usize).filter(move |&i| changes & (1 << i) != 0)
    }

}
