//! Inventory data structure storing item stacks.

use crate::item::ItemStack;
use crate::block;
use crate::item;


/// An base generic inventory with the given number of rows.
#[derive(Debug)]
pub struct Inventory<const SIZE: usize> {
    /// Rows of items in the inventory, we use a two-dimensional array in order to easily
    /// multiply the `ROWS` const generic.
    items: [ItemStack; SIZE],
    /// List of slot indices where item has changed.
    changes: u64,
}

impl<const SIZE: usize> Default for Inventory<SIZE> {
    fn default() -> Self {
        Self {
            items: [ItemStack::default(); SIZE],
            changes: 0,
        }
    }
}

impl<const SIZE: usize> Inventory<SIZE> {

    const _OK: () = assert!(SIZE < 64);

    /// Get an item at the given position.
    pub fn item(&self, index: usize) -> ItemStack {
        self.items[index]
    }

    /// Add the given item to the inventory if possible. This function returns the number
    /// of items from the stack that have been successfully added in the inventory.
    pub fn add_item(&mut self, stack: ItemStack) -> u16 {

        // Do nothing if stack size is 0 or the item is air.
        if stack.size == 0 || stack.id == block::AIR as u16 {
            return stack.size;
        }

        let item = item::from_id(stack.id);
        let mut remaining_size = stack.size;

        // Only insert our item if it has no damage.
        if stack.damage == 0 {
            // Search a slot where the item is compatible.
            for (index, slot) in self.items.iter_mut().enumerate() {

                // If the slot is of the same item and has space left in the stack size.
                if slot.id == stack.id && slot.damage == 0 && slot.size < item.max_stack_size {

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
        // We can also land here if the item has damage value.
        // We search empty slots.
        for (index, slot) in self.items.iter_mut().enumerate() {
            if slot.id == block::AIR as u16 {
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
    pub fn changes(&self) -> impl Iterator<Item = (usize, ItemStack)> + '_ {
        (0..64usize).filter_map(|i| {
            if self.changes & (1 << i) != 0 {
                Some((i, self.items[i]))
            } else {
                None
            }
        })
    }

    /// Clear changes registered in this 
    pub fn clear_changes(&mut self) {
        self.changes = 0;
    }

}
