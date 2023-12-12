//! Methods for the projectile entity data.

use super::{Projectile, ProjectileKind, EntityKind};


impl Projectile {

}

impl ProjectileKind {

    pub fn entity_kind(&self) -> EntityKind {
        match self {
            ProjectileKind::Arrow(_) => EntityKind::Arrow,
            ProjectileKind::Egg(_) => EntityKind::Egg,
            ProjectileKind::Fireball(_) => EntityKind::Fireball,
            ProjectileKind::Snowball(_) => EntityKind::Snowball,
        }
    }

}
