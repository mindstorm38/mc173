//! Player entity implementation.

use crate::world::World;

use super::{EntityLogic, Base, Living, Size};


#[derive(Debug, Default)]
pub struct Player {
    /// The player username.
    pub username: String,
    /// True when the player is sleeping.
    pub sleeping: bool,
}

/// A player entity.
pub type PlayerEntity = Base<Living<Player>>;

impl EntityLogic for PlayerEntity {
    
    fn tick(&mut self, world: &mut World) {
        
        self.update(world, Size::new(0.9, 0.9));

    }

}
