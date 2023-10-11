//! Player entity implementation.

use glam::DVec3;

use crate::world::World;

use super::{BaseEntity, LivingEntity, Entity};


#[derive(Debug)]
pub struct PlayerEntity {
    /// Base entity data.
    base: BaseEntity,
    /// Living entity data.
    living: LivingEntity,
    /// The player username.
    username: String,
}

impl PlayerEntity {

    pub fn new(pos: DVec3, username: String) -> Self {
        Self {
            base: BaseEntity::new(pos, 0.6, 1.8),
            living: LivingEntity::default(),
            username,
        }
    }

}

impl Entity for PlayerEntity {

    fn init(&mut self, id: u32) {
        self.base.id = id;
    }
    
    fn tick(&mut self, world: &mut World) {
        

    }

    fn base(&self) -> &BaseEntity {
        &self.base
    }

}
