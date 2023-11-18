//! This module contains definition and behaviors for block entities.

use glam::IVec3;

use crate::world::World;

pub mod chest;


/// All kinds of block entities.
#[derive(Debug, Clone)]
pub enum BlockEntity {
    Chest(chest::ChestBlockEntity),
    Furnace(()),
    Dispenser(()),
    Spawner(()),
    NoteBlock(()),
    Piston(()),
    Sign(()),
    Jukebox(()),
}

impl BlockEntity {

    /// Tick the block entity at its position in the world.
    pub fn tick(&mut self, world: &mut World, pos: IVec3) {
        match self {
            BlockEntity::Chest(_) => {},
            BlockEntity::Furnace(_) => {},
            BlockEntity::Dispenser(_) => {},
            BlockEntity::Spawner(_) => {},
            BlockEntity::NoteBlock(_) => {},
            BlockEntity::Piston(_) => {},
            BlockEntity::Sign(_) => {},
            BlockEntity::Jukebox(_) => {},
        }
    }

}