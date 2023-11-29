//! Spawner block entity.

use glam::IVec3;

use crate::world::World;


#[derive(Debug, Clone)]
pub struct SpawnerBlockEntity {
    /// Remaining ticks to spawn the entity.
    pub remaining_ticks: u32,
}

impl Default for SpawnerBlockEntity {

    #[inline]
    fn default() -> Self {
        Self { 
            remaining_ticks: 20,
        }
    }
    
}

impl SpawnerBlockEntity {

    /// Tick the furnace block entity.
    pub fn tick(&mut self, world: &mut World, pos: IVec3) {
        let _ = (world, pos);
        // TODO:
    }

}
