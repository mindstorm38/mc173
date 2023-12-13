//! Methods for the living entity data.

use super::{Living, LivingKind, EntityKind};


impl Living {



}

impl LivingKind {

    /// Get the generic entity kind from this living entity kind.
    pub fn entity_kind(&self) -> EntityKind {
        match self {
            LivingKind::Player(_) => EntityKind::Player,
            LivingKind::Ghast(_) => EntityKind::Ghast,
            LivingKind::Slime(_) => EntityKind::Slime,
            LivingKind::Pig(_) => EntityKind::Pig,
            LivingKind::Chicken(_) => EntityKind::Chicken,
            LivingKind::Cow(_) => EntityKind::Cow,
            LivingKind::Sheep(_) => EntityKind::Sheep,
            LivingKind::Squid(_) => EntityKind::Squid,
            LivingKind::Wolf(_) => EntityKind::Wolf,
            LivingKind::Creeper(_) => EntityKind::Creeper,
            LivingKind::Giant(_) => EntityKind::Giant,
            LivingKind::PigZombie(_) => EntityKind::PigZombie,
            LivingKind::Skeleton(_) => EntityKind::Skeleton,
            LivingKind::Spider(_) => EntityKind::Spider,
            LivingKind::Zombie(_) => EntityKind::Zombie,
        }
    }

}
