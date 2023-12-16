//! Methods for the base entity data.

use super::{Base, BaseKind, EntityKind};


impl Base {

    

}

impl BaseKind {

    /// Get the generic entity kind from this base entity kind.
    pub fn entity_kind(&self) -> EntityKind {
        match self {
            BaseKind::Item(_) => EntityKind::Item,
            BaseKind::Painting(_) => EntityKind::Painting,
            BaseKind::Boat(_) => EntityKind::Boat,
            BaseKind::Minecart(_) => EntityKind::Minecart,
            BaseKind::Fish(_) => EntityKind::Fish,
            BaseKind::LightningBolt(_) => EntityKind::LightningBolt,
            BaseKind::FallingBlock(_) => EntityKind::FallingBlock,
            BaseKind::Tnt(_) => EntityKind::Tnt,
            BaseKind::Projectile(_, kind) => kind.entity_kind(),
            BaseKind::Living(_, kind) => kind.entity_kind(),
        }
    }

}
