//! Furnace block entity.

use glam::IVec3;

use crate::smelt::find_smelting_recipe;
use crate::item::ItemStack;
use crate::world::World;


#[derive(Debug, Clone, Default)]
pub struct FurnaceBlockEntity {
    /// Input stack of the furnace.
    pub input_stack: ItemStack,
    /// Item stack for fueling the furnace.
    pub fuel_stack: ItemStack,
    /// Output stack of the furnace.
    pub output_stack: ItemStack,
    /// Current burn ticks remaining until a next fuel need to be consumed.
    pub burn_remaining_ticks: u32,
    /// Current ticks count since the current item has been added.
    pub cook_ticks: u32,
    /// Last input stack, used to compare to new one and updated the current recipe.
    last_input_stack: ItemStack,
    /// Current recipe.
    active_recipe: Option<ActiveRecipe>,
}

/// Internal cache for the active smelting recipe.
#[derive(Debug, Clone, Default)]
struct ActiveRecipe {
    /// Input stack for this recipe.
    input_stack: ItemStack,
    /// Output stack for this recipe.
    output_stack: ItemStack,
}

impl FurnaceBlockEntity {

    /// Tick the furnace block entity.
    pub fn tick(&mut self, world: &mut World, pos: IVec3) {

        // If the input stack have changed since last update, get the new recipe.
        if self.input_stack != self.last_input_stack {
            
        }

        let mut update_recipe = false;
        if let Some(active_recipe) = &self.active_recipe {
 
        }

        if self.active_recipe.is_none() {

        }

        if self.burn_remaining_ticks > 0 {

            self.burn_remaining_ticks -= 1;

            self.burn_remaining_ticks += 1;
            if self.burn_remaining_ticks == 200 {
                self.burn_remaining_ticks = 0;
            }

        }

        let _ = (world, pos);
        // TODO:



    }

}
