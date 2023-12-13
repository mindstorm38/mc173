//! Methods for the base entity data.

use std::ops::Sub;

use glam::{DVec3, Vec2};

use crate::util::BoundingBox;

use super::{Base, BaseKind, EntityKind};


impl Base {

    /// This function recompute the current bounding box from the position and the last
    /// size that was used to create it.
    pub fn update_bounding_box_from_pos(&mut self) {
        let half_width = (self.size.width / 2.0) as f64;
        let height = self.size.height as f64;
        let height_center = self.size.height_center as f64;
        self.bb = BoundingBox {
            min: self.pos - DVec3::new(half_width, height_center, half_width),
            max: self.pos + DVec3::new(half_width, height - height_center, half_width),
        };
        // Entity position and bounding are coherent.
        self.coherent = true;
    }

    /// This position recompute the current position based on the bounding box' position
    /// the size that was used to create it.
    pub fn update_pos_from_bounding_box(&mut self) {
        
        let height_center = self.size.height_center as f64;
        let new_pos = DVec3 {
            x: (self.bb.min.x + self.bb.max.x) / 2.0,
            y: self.bb.min.y + height_center,
            z: (self.bb.min.z + self.bb.max.z) / 2.0,
        };

        if new_pos != self.pos {
            self.pos = new_pos;
            self.pos_dirty = true;
        }
        
    }

    /// Modify the look angles of this entity, limited to the given step. We you need to
    /// call this function many time to reach the desired look.
    pub fn update_look_by_step(&mut self, look: Vec2, step: Vec2) {
        let look = look.rem_euclid(Vec2::splat(std::f32::consts::TAU));
        let delta = look.sub(self.look).min(step);
        if delta != Vec2::ZERO {
            self.look_dirty = true;
            self.look += delta;
        }
    }

    /// Modify the look angles to point to a given target step by step.
    pub fn update_look_at_by_step(&mut self, target: DVec3, step: Vec2) {
        let delta = target - self.pos;
        let horizontal_dist = delta.length();
        let yaw = f64::atan2(delta.z, delta.x) as f32 - std::f32::consts::FRAC_PI_2;
        let pitch = -f64::atan2(delta.y, horizontal_dist) as f32;
        self.update_look_by_step(Vec2::new(yaw, pitch), step);
    }

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
