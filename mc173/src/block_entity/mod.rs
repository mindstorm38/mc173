//! This module contains definition and behaviors for block entities.

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
