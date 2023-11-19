//! Item smelting management.

use crate::{block, item};
use crate::item::ItemStack;


/// Find a smelting recipe output from given input stack. The input stack size if ignored
/// and output stack size if how much to be produced for one input item.
pub fn find_smelting_recipe(input: ItemStack) -> Option<ItemStack> {
    for recipe in RECIPES {
        if (recipe.input.id, recipe.input.damage) == (input.id, input.damage) {
            return Some(recipe.output);
        }
    }
    None
}

const RECIPES: &'static [Recipe] = &[
    Recipe::new(ItemStack::new_block(block::IRON_ORE, 0), ItemStack::new_single(item::IRON_INGOT, 0)),
    Recipe::new(ItemStack::new_block(block::GOLD_ORE, 0), ItemStack::new_single(item::GOLD_INGOT, 0)),
    Recipe::new(ItemStack::new_block(block::DIAMOND_ORE, 0), ItemStack::new_single(item::DIAMOND, 0)),
    Recipe::new(ItemStack::new_block(block::SAND, 0), ItemStack::new_block(block::GLASS, 0)),
    Recipe::new(ItemStack::new_single(item::RAW_PORKCHOP, 0), ItemStack::new_single(item::COOKED_PORKCHOP, 0)),
    Recipe::new(ItemStack::new_single(item::RAW_FISH, 0), ItemStack::new_single(item::COOKED_FISH, 0)),
    Recipe::new(ItemStack::new_block(block::COBBLESTONE, 0), ItemStack::new_block(block::STONE, 0)),
    Recipe::new(ItemStack::new_single(item::CLAY, 0), ItemStack::new_single(item::BRICK, 0)),
    Recipe::new(ItemStack::new_block(block::CACTUS, 0), ItemStack::new_single(item::DYE, 2)),
    Recipe::new(ItemStack::new_block(block::LOG, 0), ItemStack::new_single(item::COAL, 1)),
];

/// Define a smelting recipe.
struct Recipe {
    /// The item stack that is consumed to produce the output one.
    input: ItemStack,
    /// The output stack that is produced by consuming the input one.
    output: ItemStack,
}

impl Recipe {

    const fn new(input: ItemStack, output: ItemStack) -> Self {
        Self { input, output }
    }

}
