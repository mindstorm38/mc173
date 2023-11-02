//! Inventory data structure storing item stacks.

use crate::item::ItemStack;
use crate::item;


/// An base generic inventory with the given number of rows.
#[derive(Debug, Clone)]
pub struct Inventory {
    /// Rows of items in the inventory, we use a two-dimensional array in order to easily
    /// multiply the `ROWS` const generic.
    stacks: Box<[ItemStack]>,
    /// List of slot indices where item has changed.
    changes: u64,
}

impl Inventory {

    pub fn new(size: usize) -> Self {
        assert!(size < 64);
        Self {
            stacks: vec![ItemStack::default(); size].into_boxed_slice(),
            changes: 0,
        }
    }

    /// Return the size of this inventory.
    pub fn size(&self) -> usize {
        self.stacks.len()
    }

    /// Get a slice of all stacks in this inventory.
    pub fn stacks(&self) -> &[ItemStack] {
        &self.stacks
    }

    /// Get an item at the given index.
    pub fn stack(&self, index: usize) -> ItemStack {
        self.stacks[index]
    }

    /// Set an item at the given index.
    pub fn set_stack(&mut self, index: usize, stack: ItemStack) {
        self.stacks[index] = stack;
        self.changes |= 1 << index;
    }

    /// Add the given item to the inventory if possible. This function returns the number
    /// of items from the stack that have been successfully added in the inventory.
    pub fn add_stack(&mut self, stack: ItemStack) -> u16 {

        // Do nothing if stack size is 0 or the item is air.
        if stack.is_empty() {
            return stack.size;
        }

        let item = item::from_id(stack.id);
        let mut remaining_size = stack.size;

        // Only accumulate of stack size is greater than 1.
        if item.max_stack_size > 1 {
            // Search a slot where the item is compatible.
            for (index, slot) in self.stacks.iter_mut().enumerate() {

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
        // We can also land here if the item has damage value.
        // We search empty slots.
        for (index, slot) in self.stacks.iter_mut().enumerate() {
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

    /// Clear changes registered in this 
    pub fn clear_changes(&mut self) {
        self.changes = 0;
    }

    /// Return true if this inventory has been modified since the last call to 
    /// `clear_changes`.
    pub fn has_changes(&self) -> bool {
        self.changes != 0
    }

    /// Iterate over item changes that happened in this inventory, this also returns the
    /// new item at the changed position.
    pub fn changes(&self) -> impl Iterator<Item = (usize, ItemStack)> + '_ {
        (0..64usize).filter_map(|i| {
            if self.changes & (1 << i) != 0 {
                Some((i, self.stacks[i]))
            } else {
                None
            }
        })
    }

}
