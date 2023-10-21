//! Inventory data structure storing item stacks.

use crate::item::ItemStack;
use crate::block;
use crate::item;


/// An base generic inventory with the given number of rows.
#[derive(Debug)]
pub struct Inventory<const ROWS: usize> {
    /// Rows of items in the inventory.
    pub rows: [[ItemStack; 9]; ROWS],
}

impl<const ROWS: usize> Default for Inventory<ROWS> {
    fn default() -> Self {
        Self {
            rows: [[ItemStack::default(); 9]; ROWS]
        }
    }
}

impl<const ROWS: usize> Inventory<ROWS> {

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
            for row in &mut self.rows {
                for slot in row {

                    // If the slot is of the same item and has space left in the stack size.
                    if slot.id == stack.id && slot.damage == 0 && slot.size < item.max_stack_size {

                        let available = item.max_stack_size - slot.size;
                        let to_add = available.min(remaining_size);

                        slot.size += to_add;
                        remaining_size -= to_add;

                        if remaining_size == 0 {
                            return stack.size;
                        }

                    }

                }
            }
        }

        // If we land here, some items are remaining to insert in the empty slots.
        // We can also land here if the item has damage value.
        // We search empty slots.
        for row in &mut self.rows {
            for slot in row {
                if slot.id == block::AIR as u16 {
                    // We found an empty slot, insert the whole remaining stack size.
                    *slot = stack;
                    slot.size = remaining_size;
                    return stack.size;
                }
            }
        }

        // Here we found no available slot so we return the total added size.
        stack.size - remaining_size

    }

}
