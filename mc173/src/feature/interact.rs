//! Interaction of players with blocks in the world.

use glam::IVec3;

use crate::block::material::Material;
use crate::block_entity::BlockEntity;
use crate::geom::Face;
use crate::block;

use super::{BlockEvent, Event, World};
use super::material::WorldMaterial;
use super::notify::WorldNotify;


/// Trait extension to world providing interactions methods when client clicks on a block.
pub trait WorldInteract: World {

    /// Interact with a block at given position. This function returns the interaction
    /// result to indicate if the interaction was handled, or if it was 
    /// 
    /// The second argument `breaking` indicates if the interaction originate from a 
    /// player breaking the block.
    fn interact_block(&mut self, pos: IVec3, breaking: bool) -> Interaction {
        if let Some((id, metadata)) = self.get_block(pos) {
            self.interact_block_unchecked(pos, id, metadata, breaking)
        } else {
            Interaction::None
        }
    }

    /// Internal function to handle block interaction at given position and with known
    /// block and metadata.
    fn interact_block_unchecked(&mut self, pos: IVec3, id: u8, metadata: u8, breaking: bool) -> Interaction {
        match id {
            block::BUTTON => interact_button(self, pos, metadata),
            block::LEVER => interact_lever(self, pos, metadata),
            block::TRAPDOOR => interact_trapdoor(self, pos, metadata),
            block::IRON_DOOR => true,
            block::WOOD_DOOR => interact_wood_door(self, pos, metadata),
            block::REPEATER |
            block::REPEATER_LIT => interact_repeater(self, pos, id, metadata),
            block::REDSTONE_ORE => interact_redstone_ore(self, pos),
            block::CRAFTING_TABLE => return Interaction::CraftingTable { pos },
            block::CHEST => return interact_chest(self, pos),
            block::FURNACE |
            block::FURNACE_LIT => return interact_furnace(self, pos),
            block::DISPENSER => return interact_dispenser(self, pos),
            block::NOTE_BLOCK => interact_note_block(self, pos, breaking),
            _ => return Interaction::None
        }.into()
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
    CraftingTable {
        /// Position of the crafting table being interacted.
        pos: IVec3,
    },
    /// A chest has been interacted, the front-end should interpret this and open the
    /// chest window.
    Chest {
        /// Positions of the chest block entities to connect to, from top layer in the
        /// window to bottom one. They have been checked to exists before.
        pos: Vec<IVec3>,
    },
    /// A furnace has been interacted, the front-end should interpret this and open the
    /// furnace window.
    Furnace {
        /// Position of the furnace block entity to connect to, it has been checked to
        /// exists.
        pos: IVec3,
    },
    /// A dispenser has been interacted, the front-end should interpret this and open
    /// the dispenser window.
    Dispenser {
        /// Position of the dispenser block entity to connect to, it has been checked to
        /// exists.
        pos: IVec3,
    },
}

impl From<bool> for Interaction {
    #[inline]
    fn from(value: bool) -> Self {
        if value { Self::Handled } else { Self::None }
    }
}

/// Standard implementation.
impl<W: World> WorldInteract for W { }


/// Interact with a button block.
fn interact_button(world: &mut impl World, pos: IVec3, mut metadata: u8) -> bool {
    if !block::button::is_active(metadata) {
        block::button::set_active(&mut metadata, true);
        world.set_block_notify(pos, block::BUTTON, metadata);
        world.schedule_block_tick(pos, block::BUTTON, 20);
    }
    true
}

fn interact_lever(world: &mut impl World, pos: IVec3, mut metadata: u8) -> bool {
    let active = block::lever::is_active(metadata);
    block::lever::set_active(&mut metadata, !active);
    world.set_block_notify(pos, block::LEVER, metadata);
    true
}

fn interact_trapdoor(world: &mut impl World, pos: IVec3, mut metadata: u8) -> bool {
    let active = block::trapdoor::is_open(metadata);
    block::trapdoor::set_open(&mut metadata, !active);
    world.set_block_notify(pos, block::TRAPDOOR, metadata);
    true
}

fn interact_wood_door(world: &mut impl World, pos: IVec3, mut metadata: u8) -> bool {

    if block::door::is_upper(metadata) {
        if let Some((block::WOOD_DOOR, metadata)) = world.get_block(pos - IVec3::Y) {
            interact_wood_door(world, pos - IVec3::Y, metadata);
        }
    } else {

        let open = block::door::is_open(metadata);
        block::door::set_open(&mut metadata, !open);

        world.set_block_notify(pos, block::WOOD_DOOR, metadata);

        if let Some((block::WOOD_DOOR, _)) = world.get_block(pos + IVec3::Y) {
            block::door::set_upper(&mut metadata, true);
            world.set_block_notify(pos + IVec3::Y, block::WOOD_DOOR, metadata);
        }

    }

    true

}

fn interact_repeater(world: &mut impl World, pos: IVec3, id: u8, mut metadata: u8) -> bool {
    let delay = block::repeater::get_delay(metadata);
    block::repeater::set_delay(&mut metadata, (delay + 1) % 4);
    world.set_block_notify(pos, id, metadata);
    true
}

fn interact_redstone_ore(world: &mut impl World, pos: IVec3) -> bool {
    world.set_block_notify(pos, block::REDSTONE_ORE_LIT, 0);
    false  // Notchian client lit the ore but do not mark the interaction.
}

fn interact_chest(world: &mut impl World, pos: IVec3) -> Interaction {

    let Some(BlockEntity::Chest(_)) = world.get_block_entity(pos) else {
        return Interaction::Handled
    };

    if world.is_block_opaque_cube(pos + IVec3::Y) {
        return Interaction::Handled;
    }

    for face in Face::HORIZONTAL {
        let face_pos = pos + face.delta();
        if world.is_block(face_pos, block::CHEST) && world.is_block_opaque_cube(face_pos + IVec3::Y) {
            return Interaction::Handled;
        }
    }

    let mut all_pos = vec![pos];

    // NOTE: Same order as Notchian server for parity, we also insert first or last
    // depending on the neighbor chest being on neg or pos face, like Notchian client.
    for face in [Face::NegX, Face::PosX, Face::NegZ, Face::PosZ] {
        let face_pos = pos + face.delta();
        if let Some(BlockEntity::Chest(_)) = world.get_block_entity(face_pos) {
            if face.is_neg() {
                all_pos.insert(0, face_pos);
            } else {
                all_pos.push(face_pos);
            }
        }
    }

    Interaction::Chest { pos: all_pos }

}

fn interact_furnace(world: &mut impl World, pos: IVec3) -> Interaction {
    if let Some(BlockEntity::Furnace(_)) = world.get_block_entity(pos) {
        Interaction::Furnace { pos }
    } else {
        Interaction::None
    }
}

fn interact_dispenser(world: &mut impl World, pos: IVec3) -> Interaction {
    if let Some(BlockEntity::Dispenser(_)) = world.get_block_entity(pos) {
        Interaction::Dispenser { pos }
    } else {
        Interaction::None
    }
}

fn interact_note_block(world: &mut impl World, pos: IVec3, breaking: bool) -> bool {

    let Some(BlockEntity::NoteBlock(note_block)) = world.get_block_entity_mut(pos) else {
        return true;
    };

    if !breaking {
        note_block.note = (note_block.note + 1) % 25;
    }

    let note = note_block.note;

    if !world.is_block_air(pos + IVec3::Y) {
        return true;
    }

    let instrument = match world.get_block_material(pos - IVec3::Y) {
        Material::Rock => 1,
        Material::Sand => 2,
        Material::Glass => 3,
        Material::Wood => 4,
        _ => 0,
    };

    world.push_event(Event::Block { 
        pos, 
        inner: BlockEvent::NoteBlock { 
            instrument, 
            note,
        },
    });
    
    true

}
