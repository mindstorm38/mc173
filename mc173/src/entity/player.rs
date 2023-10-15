//! Player entity implementation.

use crate::world::World;

use super::{EntityLogic, Base, Living};


#[derive(Debug, Default)]
pub struct Player {
    /// The player username.
    pub username: String,
}

/// A player entity.
pub type PlayerEntity = Base<Living<Player>>;

impl EntityLogic for PlayerEntity {
    
    fn tick(&mut self, world: &mut World) {
        let _ = world;
    }

}
