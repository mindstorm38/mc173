//! Common functions to apply modifications to entities.

use std::cell::RefCell;
use std::ops::Sub;

use glam::{DVec3, IVec3, Vec2, Vec3Swizzles};

use crate::block::material::Material;
use crate::util::{Face, BoundingBox};
use crate::world::World;
use crate::block;

use super::{Entity, Size, BaseKind, ProjectileKind, LivingKind,  Base};


/// Internal macro to make a refutable pattern assignment that just panic if refuted.
macro_rules! let_expect {
    ( $pat:pat = $expr:expr ) => {
        #[allow(irrefutable_let_patterns)]
        let $pat = $expr else {
            unreachable!("invalid argument for this function");
        };
    };
}

pub(super) use let_expect as let_expect;


// Thread local variables internally used to reduce allocation overhead.
thread_local! {
    /// Temporary entity id storage.
    pub(super) static ENTITY_ID: RefCell<Vec<u32>> = const { RefCell::new(Vec::new()) };
    /// Temporary bounding boxes storage.
    pub(super) static BOUNDING_BOX: RefCell<Vec<BoundingBox>> = const { RefCell::new(Vec::new()) };
}

/// Calculate the initial size of an entity, this is only called when not coherent.
pub fn calc_size(base_kind: &mut BaseKind) -> Size {
    match base_kind {
        BaseKind::Item(_) => Size::new_centered(0.25, 0.25),
        BaseKind::Painting(_) => Size::new(0.5, 0.5),
        BaseKind::Boat(_) => Size::new_centered(1.5, 0.6),
        BaseKind::Minecart(_) => Size::new_centered(0.98, 0.7),
        BaseKind::Fish(_) => Size::new(0.25, 0.25),
        BaseKind::LightningBolt(_) => Size::new(0.0, 0.0),
        BaseKind::FallingBlock(_) => Size::new_centered(0.98, 0.98),
        BaseKind::Tnt(_) => Size::new_centered(0.98, 0.98),
        BaseKind::Projectile(_, ProjectileKind::Arrow(_)) => Size::new(0.5, 0.5),
        BaseKind::Projectile(_, ProjectileKind::Egg(_)) => Size::new(0.5, 0.5),
        BaseKind::Projectile(_, ProjectileKind::Fireball(_)) => Size::new(1.0, 1.0),
        BaseKind::Projectile(_, ProjectileKind::Snowball(_)) => Size::new(0.5, 0.5),
        BaseKind::Living(_, LivingKind::Human(player)) => {
            if player.sleeping {
                Size::new(0.2, 0.2)
            } else {
                Size::new(0.6, 1.8)
            }
        }
        BaseKind::Living(_, LivingKind::Ghast(_)) => Size::new(4.0, 4.0),
        BaseKind::Living(_, LivingKind::Slime(slime)) => {
            let factor = slime.size as f32;
            Size::new(0.6 * factor, 0.6 * factor)
        }
        BaseKind::Living(_, LivingKind::Pig(_)) => Size::new(0.9, 0.9),
        BaseKind::Living(_, LivingKind::Chicken(_)) => Size::new(0.3, 0.4),
        BaseKind::Living(_, LivingKind::Cow(_)) => Size::new(0.9, 1.3),
        BaseKind::Living(_, LivingKind::Sheep(_)) =>Size::new(0.9, 1.3),
        BaseKind::Living(_, LivingKind::Squid(_)) => Size::new(0.95, 0.95),
        BaseKind::Living(_, LivingKind::Wolf(_)) => Size::new(0.8, 0.8),
        BaseKind::Living(_, LivingKind::Creeper(_)) => Size::new(0.6, 1.8),
        BaseKind::Living(_, LivingKind::Giant(_)) => Size::new(3.6, 10.8),
        BaseKind::Living(_, LivingKind::PigZombie(_)) => Size::new(0.6, 1.8),
        BaseKind::Living(_, LivingKind::Skeleton(_)) => Size::new(0.6, 1.8),
        BaseKind::Living(_, LivingKind::Spider(_)) => Size::new(1.4, 0.9),
        BaseKind::Living(_, LivingKind::Zombie(_)) => Size::new(0.6, 1.8),
    }
}

/// Calculate height height for the given entity.
pub fn calc_eye_height(base: &Base, base_kind: &BaseKind) -> f32 {
    match base_kind {
        BaseKind::Living(_, LivingKind::Human(_)) => 1.62,
        BaseKind::Living(_, LivingKind::Wolf(_)) => base.size.height * 0.8,
        BaseKind::Living(_, _) => base.size.height * 0.85,
        _ => 0.0,
    }
}

/// Calculate the eye position of the given entity.
pub fn calc_eye_pos(base: &Base) -> DVec3 {
    let mut pos = base.pos;
    pos.y += base.eye_height as f64;
    pos
}

/// Calculate the velocity of a fluid at given position, this depends on neighbor blocks.
/// This calculation will only take the given material into account, this material should
/// be a fluid material (water/lava), and the given metadata should be the one of the
/// current block the the position.
pub fn calc_fluid_vel(world: &World, pos: IVec3, material: Material, metadata: u8) -> DVec3 {

    debug_assert!(material.is_fluid());

    let distance = block::fluid::get_actual_distance(metadata);
    let mut vel = DVec3::ZERO;

    for face in Face::HORIZONTAL {

        let face_delta = face.delta();
        let face_pos = pos + face_delta;
        let (face_block, face_metadata) = world.get_block(face_pos).unwrap_or_default();
        let face_material = block::material::get_material(face_block);

        if face_material == material {
            let face_distance = block::fluid::get_actual_distance(face_metadata);
            let delta = face_distance as i32 - distance as i32;
            vel += (face_delta * delta).as_dvec3();
        } else if !face_material.is_solid() {
            let below_pos = face_pos - IVec3::Y;
            let (below_block, below_metadata) = world.get_block(below_pos).unwrap_or_default();
            let below_material = block::material::get_material(below_block);
            if below_material == material {
                let below_distance = block::fluid::get_actual_distance(below_metadata);
                let delta = below_distance as i32 - (distance as i32 - 8);
                vel += (face_delta * delta).as_dvec3();
            }
        }

    }

    // TODO: Things with falling water.

    vel.normalize()

}

pub fn calc_entity_brightness(world: &World, base: &Base) -> f32 {
    let mut check_pos = base.pos;
    check_pos.y += (base.size.height * 0.66 - base.size.center) as f64;
    world.get_brightness(check_pos.floor().as_ivec3()).unwrap_or(0.0)
}

pub fn find_closest_player_entity(world: &World, center: DVec3, dist: f64) -> Option<(u32, &Entity)> {
    let max_dist_sq = dist.powi(2);
    world.iter_entities()
        .filter(|(_, entity)| matches!(entity.1, BaseKind::Living(_, LivingKind::Human(_))))
        .map(|(entity_id, entity)| (entity_id, entity, entity.0.pos.distance_squared(center)))
        .filter(|&(_, _, dist_sq)| dist_sq <= max_dist_sq)
        .min_by(|(_, _, a), (_, _, b)| a.total_cmp(b))
        .map(|(entity_id, entity, _)| (entity_id, entity))
}

/// This function recompute the current bounding box from the position and the last
/// size that was used to create it.
pub fn update_bounding_box_from_pos(base: &mut Base) {
    let half_width = (base.size.width / 2.0) as f64;
    let height = base.size.height as f64;
    let center = base.size.center as f64;
    base.bb = BoundingBox {
        min: base.pos - DVec3::new(half_width, center, half_width),
        max: base.pos + DVec3::new(half_width, height - center, half_width),
    };
    // Entity position and bounding are coherent.
    base.coherent = true;
}

/// This position recompute the current position based on the bounding box' position
/// the size that was used to create it.
pub fn update_pos_from_bounding_box(base: &mut Base) {
    let center = base.size.center as f64;
    base.pos = DVec3 {
        x: (base.bb.min.x + base.bb.max.x) / 2.0,
        y: base.bb.min.y + center,
        z: (base.bb.min.z + base.bb.max.z) / 2.0,
    };
}

/// Modify the look angles of this entity, limited to the given step. 
/// We need to call this function many time to reach the desired look.
pub fn update_look_by_step(base: &mut Base, look: Vec2, step: Vec2) {
    
    let look_norm = Vec2 {
        // Yaw can be normalized between 0 and tau
        x: look.x.rem_euclid(std::f32::consts::TAU),
        // Pitch however is not normalized.
        y: look.y,
    };

    base.look += look_norm.sub(base.look).min(step);

}

/// Modify the look angles to point to a given target step by step. The eye height is
/// included in the calculation in order to make the head looking at target.
pub fn update_look_at_by_step(base: &mut Base, target: DVec3, step: Vec2) {
    let delta = target - calc_eye_pos(base);
    let yaw = f64::atan2(delta.z, delta.x) as f32 - std::f32::consts::FRAC_PI_2;
    let pitch = -f64::atan2(delta.y, delta.xz().length()) as f32;
    update_look_by_step(base, Vec2::new(yaw, pitch), step);
}

/// Almost the same as [`update_look_at_by_step`] but the target is another entity base,
/// this function will make the entity look at the eyes of the target one.
pub fn update_look_at_entity_by_step(base: &mut Base, target_base: &Base, step: Vec2) {
    update_look_at_by_step(base, calc_eye_pos(target_base), step);
}

/// Apply knock back to this entity's velocity.
pub fn update_knock_back(base: &mut Base, dir: DVec3) {

    let mut accel = dir.normalize_or_zero();
    accel.y -= 1.0;

    base.vel /= 2.0;
    base.vel -= accel * 0.4;
    base.vel.y = base.vel.y.min(0.4);

}

/// Return true if the entity can eye track the target entity, this use ray tracing.
pub fn can_eye_track(world: &World, base: &Base, target_base: &Base) -> bool {
    let origin = calc_eye_pos(base);
    let ray = calc_eye_pos(target_base) - origin;
    world.ray_trace_blocks(origin, ray, false).is_none()
}
