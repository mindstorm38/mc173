//! Furnace block entity.

use glam::IVec3;

use crate::item::{self, ItemStack};
use crate::world::World;
use crate::{smelt, block};


#[derive(Debug, Clone, Default)]
pub struct FurnaceBlockEntity {
    /// Input stack of the furnace.
    pub input_stack: ItemStack,
    /// Item stack for fueling the furnace.
    pub fuel_stack: ItemStack,
    /// Output stack of the furnace.
    pub output_stack: ItemStack,
    /// Max burn ticks for the current fuel being consumed.
    pub burn_max_ticks: u16,
    /// Current burn remaining ticks until a fuel item need to be consumed again.
    pub burn_remaining_ticks: u16,
    /// Current ticks count since the current item has been added.
    pub smelt_ticks: u16,
    /// Last input stack, used to compare to new one and updated the current recipe.
    last_input_stack: ItemStack,
    /// If some recipe has been found for the current input stack, this contains the
    /// future output stack that will be assigned to output stack.
    active_output_stack: Option<ItemStack>,
}

impl FurnaceBlockEntity {

    /// Internal function to compute the new recipe depending on the current input item.
    /// None is returned if the input stack is empty, if no recipe can be found, or if
    /// the recipe's output do not fit in the output stack.
    fn find_new_output_stack(&self) -> Option<ItemStack> {

        if self.input_stack.size == 0 {
            return None;
        }

        let input_id = self.input_stack.id;
        let input_damage = self.input_stack.damage;
        let output_stack = smelt::find_smelting_output(input_id, input_damage)?;

        if !self.output_stack.is_empty() {
            if (self.output_stack.id, self.output_stack.damage) != (output_stack.id, output_stack.damage) {
                return None;
            } else if self.output_stack.size + output_stack.size > item::from_id(output_stack.id).max_stack_size {
                return None;
            }
        }

        Some(output_stack)

    }

    /// Tick the furnace block entity.
    pub fn tick(&mut self, world: &mut World, pos: IVec3) {

        // If the input stack have changed since last update, get the new recipe.
        if self.input_stack != self.last_input_stack {
            self.active_output_stack = self.find_new_output_stack();
            self.last_input_stack = self.input_stack;
        }

        let initial_burning = self.burn_remaining_ticks != 0;
        if initial_burning {
            self.burn_remaining_ticks -= 1;
        }

        if let Some(active_output_stack) = &self.active_output_stack {

            if self.burn_remaining_ticks == 0 && !self.fuel_stack.is_empty() {
                self.burn_max_ticks = smelt::get_burn_ticks(self.fuel_stack.id);
                self.burn_remaining_ticks = self.burn_max_ticks;
                if self.burn_max_ticks != 0 {
                    self.fuel_stack.size -= 1;
                }
            }

            if self.burn_remaining_ticks == 0 {
                self.smelt_ticks = 0;
                // TODO: Event
            } else {

                self.smelt_ticks += 1;
                if self.smelt_ticks == 200 {
                    self.smelt_ticks = 0;
                    // This should not underflow because if input stack is empty, not
                    // active output stack can be set.
                    // NOTE: Modifying both of these will trigger an update of the active 
                    // output stack on the next tick.
                    self.input_stack.size -= 1;
                    self.output_stack = *active_output_stack;
                }

                // TODO: Event

            }
            
        } else {
            self.smelt_ticks = 0;
            // TODO: Event
        }

        if initial_burning != (self.burn_remaining_ticks != 0) {
            if initial_burning {
                // No longer burning.
                world.set_block_notify(pos, block::FURNACE, 0);
            } else {
                // Now burning.
                world.set_block_notify(pos, block::FURNACE_LIT, 0);
            }
        } 

    }

}
