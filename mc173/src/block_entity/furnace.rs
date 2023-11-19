//! Furnace block entity.

use glam::IVec3;

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
}

impl FurnaceBlockEntity {

    /// Tick the furnace block entity.
    pub fn tick(&mut self, world: &mut World, pos: IVec3) {
        let _ = (world, pos);
        // TODO:
    }

}
