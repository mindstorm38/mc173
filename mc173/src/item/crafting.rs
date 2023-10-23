//! Item crafting management.

use crate::item::inventory::Inventory;
use crate::item::{self, ItemStack};
use crate::block;


/// This structure keeps track of the current crafting recipe selected and allows lazy
/// update of the crafting recipe.
#[derive(Debug, Default)]
pub struct CraftingTracker {
    /// The index and result item of the current selected recipe
    current_recipe: Option<(usize, ItemStack)>,
}

impl CraftingTracker {

    /// Update this tracker to track a new grid of items.
    pub fn update(&mut self, inv: &Inventory, width: u8, height: u8) {
        
        let stacks_count = width as usize * height as usize;
        assert_eq!(stacks_count, inv.size(), "incoherent inventory size");

        self.current_recipe = None;

        for (recipe_index, recipe) in RECIPES.iter().enumerate() {
            
            let item = match recipe {
                Recipe::Shaped(shaped) => shaped.check(inv, width as usize, height as usize),
                Recipe::Shapeless(shapeless) => shapeless.check(inv),
            };

            if let Some(item) = item {
                self.current_recipe = Some((recipe_index, item));
                break;
            }

        }

    }

    /// If a crafting recipe is currently selected, return the result item.
    pub fn recipe(&self) -> Option<ItemStack> {
        self.current_recipe.map(|(_, item)| item)
    }

}


// Here are the stack definitions of crafting recipes.

macro_rules! def_const_stack {
    ( $( $name:ident = $value:expr; )* ) => {
        $( const $name: ItemStack = ItemStack { id: $value as u16, size: 1, damage: 0 }; )*
    };
}

def_const_stack! {
    SUGAR_CANES     = item::SUGAR_CANES;
    PAPER           = item::PAPER;
    BOOK            = item::BOOK;
    STICK           = item::STICK;
    FENCE           = block::FENCE;
    DIAMOND         = item::DIAMOND;
    WOOD            = block::WOOD;
    JUKEBOX         = block::JUKEBOX;
    REDSTONE        = item::REDSTONE;
    NOTE_BLOCK      = block::NOTE_BLOCK;
    BOOKSHELF       = block::BOOKSHELF;
    SNOWBALL        = item::SNOWBALL;
    SNOW_BLOCK      = block::SNOW_BLOCK;
    CLAY            = item::CLAY;
    CLAY_BLOCK      = block::CLAY;
    BRICK           = item::BRICK;
    BRICK_BLOCK     = block::BRICK;
    GLOWSTONE_DUST  = item::GLOWSTONE_DUST;
    GLOWSTONE       = block::GLOWSTONE;
    STRING          = item::STRING;
    WOOL            = block::WOOL;
    GUNPOWDER       = item::GUNPOWDER;
    SAND            = block::SAND;
    TNT             = block::TNT;

    BED             = item::BED;
}

const RECIPES: &'static [Recipe] = &[
    Recipe::new_shaped(PAPER, &[SUGAR_CANES, SUGAR_CANES, SUGAR_CANES], 3),
    Recipe::new_shaped(BOOK, &[PAPER, PAPER, PAPER], 1),
    Recipe::new_shaped(FENCE, &[STICK, STICK, STICK, STICK, STICK, STICK], 3),
    Recipe::new_shaped(JUKEBOX, &[WOOD, WOOD, WOOD, WOOD, DIAMOND, WOOD, WOOD, WOOD, WOOD], 3),
    Recipe::new_shaped(NOTE_BLOCK, &[WOOD, WOOD, WOOD, WOOD, REDSTONE, WOOD, WOOD, WOOD, WOOD], 3),
    Recipe::new_shaped(BOOKSHELF, &[WOOD, WOOD, WOOD, BOOK, BOOK, BOOK, WOOD, WOOD, WOOD], 3),
    Recipe::new_shaped(SNOW_BLOCK, &[SNOWBALL, SNOWBALL, SNOWBALL, SNOWBALL], 2),
    Recipe::new_shaped(CLAY_BLOCK, &[CLAY, CLAY, CLAY, CLAY], 2),
    Recipe::new_shaped(BRICK_BLOCK, &[BRICK, BRICK, BRICK, BRICK], 2),
    Recipe::new_shaped(GLOWSTONE, &[GLOWSTONE_DUST, GLOWSTONE_DUST, GLOWSTONE_DUST, GLOWSTONE_DUST], 2),
    Recipe::new_shaped(WOOL, &[STRING, STRING, STRING, STRING], 2),
    Recipe::new_shaped(TNT, &[GUNPOWDER, SAND, GUNPOWDER, SAND, GUNPOWDER, SAND, GUNPOWDER, SAND, GUNPOWDER], 3),

    Recipe::new_shaped(BED, &[WOOL, WOOL, WOOL, WOOD, WOOD], 3),
];


/// The recipe enumeration stores different types of recipes.
enum Recipe {
    /// A shaped crafting recipe requires the items to be in a specific pattern, the 
    /// pattern has a size and if smaller than 3x3 it can be moved everywhere in the 
    /// table.
    Shaped(ShapedRecipe),
    /// A shapeless crafting just define a list of items that must be present in the
    /// crafting grid, each stack must be present once.
    Shapeless(ShapelessRecipe),
}

struct ShapedRecipe {
    result: ItemStack,
    pattern: &'static [ItemStack],
    width: u8,
}

struct ShapelessRecipe {
    result: ItemStack,
    pattern: &'static [ItemStack],
}

impl Recipe {
    
    const fn new_shaped(result: ItemStack, pattern: &'static [ItemStack], width: u8) -> Self {
        Self::Shaped(ShapedRecipe { result, pattern, width })
    }

    const fn new_shapeless(result: ItemStack, pattern: &'static [ItemStack]) -> Self {
        Self::Shapeless(ShapelessRecipe { result, pattern })
    }

}

impl ShapedRecipe {

    /// Check if this shaped recipe can be crafted with the given inventory of the given 
    /// size and items.
    fn check(&self, inv: &Inventory, inv_width: usize, inv_height: usize) -> Option<ItemStack> {

        // Compute recipe size based on pattern length and width.
        // NOTE: We compute the height in which the pattern fit.
        let recipe_width = self.width as usize;
        let recipe_height = (self.pattern.len() + recipe_width - 1) / recipe_width;

        // Recipe size cannot fit in the given inventory shape: discard immediately.
        // NOTE: This also avoids arithmetics underflow just below.
        if recipe_width > inv_width || recipe_height > inv_height {
            return None;
        }

        // For each possible starting point in the inventory, check.
        for start_x in 0..=(inv_width - recipe_width) {
            for start_y in 0..=(inv_height - recipe_height) {
                if self.check_at(inv, inv_width, start_x, start_y, false) {
                    return Some(self.result);
                } else if self.check_at(inv, inv_width, start_x, start_y, true) {
                    return Some(self.result);
                }
            }
        }

        None

    }

    /// Internal function to check if this recipe can be crafted at the given coordinates
    /// in the inventory. The `flip` argument is used to check the pattern but flipped.
    fn check_at(&self, inv: &Inventory, inv_width: usize, start_x: usize, start_y: usize, flip: bool) -> bool {
        
        let recipe_width = self.width as usize;

        let mut dx = if flip { recipe_width - 1 } else { 0 };
        let mut dy = 0;

        for pat_stack in self.pattern {

            let stack = inv.stack((start_x + dx) + (start_y + dy) * inv_width);
            if pat_stack.is_empty() && !stack.is_empty() {
                return false;
            } else if (pat_stack.id, pat_stack.damage) != (stack.id, stack.damage) {
                return false;
            } else if stack.size < pat_stack.size {
                return false;
            }

            if flip {
                if dx == 0 {
                    dy += 1;
                    dx = recipe_width - 1;
                } else {
                    dx -= 1;
                }
            } else {
                dx += 1;
                if dx >= recipe_width {
                    dy += 1;
                    dx = 0;
                }
            }

        }

        true

    }

}

impl ShapelessRecipe {

    /// Check if this shapeless recipe can be crafted with the given inventory items.
    /// 
    /// **Note that shapeless crafting currently ignore the stack size in of pattern.**
    fn check(&self, inv: &Inventory) -> Option<ItemStack> {
        
        // Too few stacks for the current pattern: discard immediately.
        if inv.size() < self.pattern.len() {
            return None;
        }

        // We use a single integer to mark inv items that have been found.
        let mut inv_matched = 0u32;

        'pat: for pat_stack in self.pattern {
            for (i, stack) in inv.stacks().iter().copied().enumerate() {
                if inv_matched & (1 << i) ==  0 {
                    if (pat_stack.id, pat_stack.damage) == (stack.id, stack.damage) {
                        inv_matched |= 1 << i;
                        continue 'pat;
                    }
                }
            }
            // If we land here, we did not found the required item.
            return None;
        }

        Some(self.result)

    }

}
