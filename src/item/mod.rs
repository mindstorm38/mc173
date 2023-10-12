//! Item enumeration and behaviors.

use crate::block::block_from_id;


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
pub fn item_from_id(id: u16) -> &'static Item {
    if id < 256 {
        &block_from_id(id as u8).item
    } else {
        &ITEMS[(id - 256) as usize]
    }
}


/// This structure describe a block.
#[derive(Debug, Clone, Copy)]
pub struct Item {
    /// The name of the item, used for debug purpose.
    pub name: &'static str,
    /// Maximum stack size for this item.
    pub max_stack: u16,
}

impl Item {

    pub const fn new(name: &'static str) -> Self {
        Self {
            name,
            max_stack: 64,
        }
    }

}


#[derive(Debug, Clone, Default)]
pub struct ItemStack {
    pub id: u8,
    pub damage: u8,
    pub stack: u16,
}
