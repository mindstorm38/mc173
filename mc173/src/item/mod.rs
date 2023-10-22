//! Item enumeration and behaviors.

use crate::block;

pub mod interact;


/// Internal macro to easily define blocks registry.
macro_rules! items {
    (
        $($name:ident / $id:literal : $init:expr),* $(,)?
    ) => {

        static ITEMS: [Item; 256] = {
            let mut arr = [Item::new("undefined"); 256];
            $(arr[$id as usize] = $init;)*
            arr
        };

        $(pub const $name: u16 = $id + 256;)*

    };
}

items! {
    IRON_SHOVEL/0:      Item::new("iron_shovel"),
    IRON_PICKAXE/1:     Item::new("iron_pickaxe"),
    IRON_AXE/2:         Item::new("iron_axe"),
    FLINT_AND_STEEL/3:  Item::new("flint_and_steel"),
}


/// Get an item from its numeric id.
pub fn from_id(id: u16) -> &'static Item {
    if id < 256 {
        &block::from_id(id as u8).item
    } else {
        &ITEMS[(id - 256) as usize]
    }
}


/// This structure describe a block.
#[derive(Debug, Clone, Copy)]
pub struct Item {
    /// The name of the item, used for debug purpose.
    pub name: &'static str,
    /// Set to true if this item is derived from a block.
    pub block: bool,
    /// Maximum stack size for this item.
    pub max_stack_size: u16,
}

impl Item {

    pub const fn new(name: &'static str) -> Self {
        Self {
            name,
            block: false,
            max_stack_size: 64,
        }
    }

}


/// An item stack defines the actual number of items and their damage value.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ItemStack {
    /// The item id.
    pub id: u16,
    /// The stack size.
    pub size: u16,
    /// The damage value of the stack.
    pub damage: u16,
}

impl ItemStack {

    pub const EMPTY: Self = Self { id: block::AIR as u16, size: 0, damage: 0 };

    pub fn with_size(mut self, size: u16) -> ItemStack {
        self.size = size;
        self
    }

    pub fn with_damage(mut self, damage: u16) -> ItemStack {
        self.damage = damage;
        self
    }

    /// Return true if this item stack is air, which is a special case where the item 
    /// stack represent an empty slot.
    pub fn is_empty(self) -> bool {
        self.id == block::AIR as u16 || self.size == 0
    }

    /// Simplify this item stack by converting it into `None` if the item is just a air
    /// block, which is equivalent to no item for Minecraft, regardless of the damage 
    /// value or stack size.
    pub fn to_non_empty(self) -> Option<ItemStack> {
        if self.is_empty() {
            None
        } else {
            Some(self)
        }
    }

}
