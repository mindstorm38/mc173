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

    #[inline]
    pub fn get(&self, index: usize) -> ItemStack {
        self.inv[index]
    }

    #[inline]
    pub fn set(&mut self, index: usize, stack: ItemStack) {
        self.inv[index] = stack;
        self.changes |= 1 << index;
    }

    /// Add an item to the inventory. The given item stack is modified according to the
    /// amount of items actually added to the inventory, its size will be set to zero if
    /// fully consumed.
    pub fn add(&mut self, stack: &mut ItemStack) {

        // Do nothing if stack size is 0 or the item is air.
        if stack.is_empty() {
            return;
        }

        let item = item::from_id(stack.id);

        // Only accumulate of stack size is greater than 1.
        if item.max_stack_size > 1 {
            // Search a slot where the item is compatible.
            for (index, slot) in self.inv.iter_mut().enumerate() {
                // If the slot is of the same item and has space left in the stack size.
                if slot.id == stack.id && slot.damage == stack.damage && slot.size < item.max_stack_size {
                    let available = item.max_stack_size - slot.size;
                    let to_add = available.min(stack.size);
                    slot.size += to_add;
                    stack.size -= to_add;
                    // NOTE: We requires that size must be less than 64, so the index fit
                    // in the 64 bits of changes integer.
                    self.changes |= 1 << index;
                    if stack.size == 0 {
                        return;
                    }
                }
            }
        }

        // If we land here, some items are remaining to insert in the empty slots.
        // We can also land here if the item has damage value. We search empty slots.
        for (index, slot) in self.inv.iter_mut().enumerate() {
            if slot.is_empty() {
                // We found an empty slot, insert the whole remaining stack size.
                *slot = *stack;
                stack.size = 0;
                self.changes |= 1 << index;
                return;
            }
        }
        
    }

    /// Consume the equivalent of the given item stack, returning true if successful.
    pub fn consume(&mut self, stack: ItemStack) -> bool {
        
        for (index, slot) in self.inv.iter_mut().enumerate() {
            if slot.id == stack.id && slot.damage == stack.damage && slot.size >= stack.size {
                slot.size -= stack.size;
                self.changes |= 1 << index;
                return true;
            }
        }
        
        false

    }

    /// Iterate over item changes that happened in this inventory, this also returns the
    /// new item at the changed position.
    pub fn iter_changes(&self) -> impl Iterator<Item = usize> {
        let changes = self.changes;
        (0..64usize).filter(move |&i| changes & (1 << i) != 0)
    }

}
