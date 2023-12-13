//! Methods for the projectile entity data.

use super::{Projectile, ProjectileKind, EntityKind};


impl Projectile {

}

impl ProjectileKind {

    /// Get the generic entity kind from this projectile entity kind.
    pub fn entity_kind(&self) -> EntityKind {
        match self {
            ProjectileKind::Arrow(_) => EntityKind::Arrow,
            ProjectileKind::Egg(_) => EntityKind::Egg,
            ProjectileKind::Fireball(_) => EntityKind::Fireball,
            ProjectileKind::Snowball(_) => EntityKind::Snowball,
        }
    }

}
