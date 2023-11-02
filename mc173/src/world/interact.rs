//! Interaction of players with blocks in the world.

use glam::IVec3;

use crate::block;

use super::World;


impl World {

    /// Interact with a block at given position. This function returns true if an 
    /// interaction has been handled and some action happened to the world, which should
    /// typically prevent usage of the player's hand item.
    pub fn interact_block(&mut self, pos: IVec3) -> Interaction {
        if let Some((id, metadata)) = self.get_block(pos) {
            self.handle_interact_block(pos, id, metadata)
        } else {
            Interaction::None
        }
    }

    /// Internal function to handle block interaction at given position and with known
    /// block and metadata. The function returns true if an interaction has been handled.
    pub(super) fn handle_interact_block(&mut self, pos: IVec3, id: u8, metadata: u8) -> Interaction {
        match id {
            block::BUTTON => self.interact_button(pos, metadata),
            block::LEVER => self.interact_lever(pos, metadata),
            block::TRAPDOOR => self.interact_trapdoor(pos, metadata),
            block::IRON_DOOR => true,
            block::WOOD_DOOR => self.interact_wood_door(pos, metadata),
            block::REPEATER |
            block::REPEATER_LIT => self.interact_repeater(pos, id, metadata),
            block::REDSTONE_ORE => self.interact_redstone_ore(pos),
            block::CRAFTING_TABLE => return Interaction::CraftingTable,
            _ => return Interaction::None
        }.into()
    }

    /// Interact with a button block.
    fn interact_button(&mut self, pos: IVec3, mut metadata: u8) -> bool {
        if !block::button::is_active(metadata) {
            block::button::set_active(&mut metadata, true);
            self.set_block_notify(pos, block::BUTTON, metadata);
            self.schedule_tick(pos, block::BUTTON, 20);
        }
        true
    }

    fn interact_lever(&mut self, pos: IVec3, mut metadata: u8) -> bool {
        let active = block::lever::is_active(metadata);
        block::lever::set_active(&mut metadata, !active);
        self.set_block_notify(pos, block::LEVER, metadata);
        true
    }

    fn interact_trapdoor(&mut self, pos: IVec3, mut metadata: u8) -> bool {
        let active = block::trapdoor::is_open(metadata);
        block::trapdoor::set_open(&mut metadata, !active);
        self.set_block_notify(pos, block::TRAPDOOR, metadata);
        true
    }

    fn interact_wood_door(&mut self, pos: IVec3, mut metadata: u8) -> bool {

        if block::door::is_upper(metadata) {
            if let Some((block::WOOD_DOOR, metadata)) = self.get_block(pos - IVec3::Y) {
                self.interact_wood_door(pos - IVec3::Y, metadata);
            }
        } else {

            let open = block::door::is_open(metadata);
            block::door::set_open(&mut metadata, !open);

            self.set_block_notify(pos, block::WOOD_DOOR, metadata);

            if let Some((block::WOOD_DOOR, _)) = self.get_block(pos + IVec3::Y) {
                block::door::set_upper(&mut metadata, true);
                self.set_block_notify(pos + IVec3::Y, block::WOOD_DOOR, metadata);
            }

        }

        true

    }

    fn interact_repeater(&mut self, pos: IVec3, id: u8, mut metadata: u8) -> bool {
        let delay = block::repeater::get_delay(metadata);
        block::repeater::set_delay(&mut metadata, (delay + 1) % 4);
        self.set_block_notify(pos, id, metadata);
        true
    }

    fn interact_redstone_ore(&mut self, pos: IVec3) -> bool {
        self.set_block_notify(pos, block::REDSTONE_ORE_LIT, 0);
        false  // Notchian client lit the ore but do not mark the interaction.
    }

}


/// The result of an interaction with a block in the world.
#[derive(Debug, Clone)]
pub enum Interaction {
    /// No interaction has been handled.
    None,
    /// An interaction has been handled by the world.
    Handled,
    /// A crafting table has been interacted, the front-end should interpret this and 
    /// open the crafting table window.
    CraftingTable,
    /// A chest has been interacted, the front-end should interpret this and open the
    /// chest window.
    Chest,
    /// A furnace has been interacted, the front-end should interpret this and open the
    /// furnace window.
    Furnace,
    /// A dispenser has been interacted, the front-end should interpret this and open
    /// the dispenser window.
    Dispenser,
}

impl From<bool> for Interaction {
    #[inline]
    fn from(value: bool) -> Self {
        if value { Self::Handled } else { Self::None }
    }
}
