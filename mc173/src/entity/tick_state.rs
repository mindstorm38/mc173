//! Tick state of the entity.

use std::ops::Add;

use glam::DVec3;

use crate::entity::{Hurt, LivingKind, ProjectileKind};
use crate::world::{World, Event, EntityEvent};
use crate::block::material::Material;
use crate::block;

use super::common::{self, let_expect};
use super::{Entity, BaseKind};


/// Tick base method that is common to every entity kind, this is split in Notchian impl
/// so we split it here.
pub(super) fn tick_state(world: &mut World, id: u32, entity: &mut Entity) {
    match entity {
        Entity(_, BaseKind::Living(_, _)) => tick_state_living(world, id, entity),
        Entity(_, _) => tick_state_base(world, id, entity),
    }
}

/// REF: Entity::onEntityUpdate
fn tick_state_base(world: &mut World, id: u32, entity: &mut Entity) {
        
    let Entity(base, base_kind) = entity;

    // Compute the bounding box used for water collision, it depends on the entity kind.
    let water_bb = match base_kind {
        BaseKind::Item(_) => base.bb,
        _ => base.bb.inflate(DVec3::new(-0.001, -0.4 - 0.001, -0.001)),
    };

    // Search for water block in the water bb.
    base.in_water = false;
    let mut water_vel = DVec3::ZERO;
    for (pos, block, metadata) in world.iter_blocks_in_box(water_bb) {
        let material = block::material::get_material(block);
        if material == Material::Water {
            let height = block::fluid::get_actual_height(metadata);
            if water_bb.max.y.add(1.0).floor() >= pos.y as f64 + height as f64 {
                base.in_water = true;
                water_vel += common::calc_fluid_vel(world, pos, material, metadata);
            }
        }
    }

    // Finalize normalisation and apply if not zero.
    let water_vel = water_vel.normalize_or_zero();
    if water_vel != DVec3::ZERO {
        base.vel += water_vel * 0.014;
        base.vel_dirty = true;
    }

    // Extinguish and cancel fall if in water.
    if base.in_water {
        base.fire_time = 0;
        base.fall_distance = 0.0;
    } else if base.fire_immune {
        base.fire_time = 0;
    }

    if base.fire_time != 0 {
        if false { // if fire immune
            base.fire_time = base.fire_time.saturating_sub(4);
        } else {
            if base.fire_time % 20 == 0 {
                // TODO: Damage entity
            }
            base.fire_time -= 1;
        }
    }

    // Check if there is a lava block colliding...
    let lava_bb = base.bb.inflate(DVec3::new(-0.1, -0.4, -0.1));
    base.in_lava = world.iter_blocks_in_box(lava_bb)
        .any(|(_, block, _)| block::material::get_material(block) == Material::Lava);

    // If this entity can pickup other ones, trigger an event.
    if base.can_pickup {

        // Temporarily owned vector to avoid allocation.
        common::ENTITY_ID.with_borrow_mut(|picked_up_entities| {

            debug_assert!(picked_up_entities.is_empty());
            
            for (entity_id, entity) in world.iter_entities_colliding(base.bb.inflate(DVec3::new(1.0, 0.0, 1.0))) {

                match &entity.1 {
                    BaseKind::Item(item) => {
                        if item.frozen_time == 0 {
                            picked_up_entities.push(entity_id);
                        }
                    }
                    BaseKind::Projectile(projectile, ProjectileKind::Arrow(arrow)) => {
                        if projectile.state.is_some() && arrow.from_player {
                            picked_up_entities.push(entity_id);
                        }
                    }
                    _ => {}
                }
            }

            for entity_id in picked_up_entities.drain(..) {
                world.push_event(Event::Entity { 
                    id, 
                    inner: EntityEvent::Pickup { 
                        target_id: entity_id,
                    },
                });
            }

        });

    }

}

/// REF: EntityLiving::onEntityUpdate
fn tick_state_living(world: &mut World, id: u32, entity: &mut Entity) {

    // Super call.
    tick_state_base(world, id, entity);

    let_expect!(Entity(base, BaseKind::Living(living, living_kind)) = entity);
    
    // Suffocate entities if inside opaque cubes (except for sleeping players).
    let mut check_suffocate = true;
    if let LivingKind::Human(human) = living_kind {
        check_suffocate = !human.sleeping;
    }

    if check_suffocate {
        for i in 0u8..8 {
            
            let delta = DVec3 {
                x: (((i >> 0) & 1) as f64 - 0.5) * base.size.width as f64 * 0.9,
                y: (((i >> 1) & 1) as f64 - 0.5) * 0.1 + base.eye_height as f64,
                z: (((i >> 2) & 1) as f64 - 0.5) * base.size.width as f64 * 0.9,
            };

            if world.is_block_opaque_cube(base.pos.add(delta).floor().as_ivec3()) {
                // One damage per tick (not overwriting if already set to higher).
                base.hurt.push(Hurt {
                    damage: 1,
                    origin_id: None,
                });
                break;
            }

        }
    }

    // TODO: Air time underwater

    // Decrease countdowns.
    living.hurt_time = living.hurt_time.saturating_sub(1);

    /// The hurt time when hit for the first time.
    /// PARITY: The Notchian impl doesn't actually use hurt time but another variable
    ///  that have the exact same behavior, so we use hurt time here to be more,
    ///  consistent. We also avoid the divide by two thing that is useless.
    const HURT_INITIAL_TIME: u16 = 10;

    while let Some(hurt) = base.hurt.pop() {

        // Don't go further if entity is already dead.
        if living.health == 0 {
            break;
        }

        // Calculate the actual damage dealt on this tick depending on cooldown.
        let mut actual_damage = 0;
        if living.hurt_time == 0 {
            
            living.hurt_time = HURT_INITIAL_TIME;
            living.hurt_last_damage = hurt.damage;
            actual_damage = hurt.damage;
            world.push_event(Event::Entity { id, inner: EntityEvent::Damage });

            if let Some(origin_id) = hurt.origin_id {
                if let Some(Entity(origin_base, _)) = world.get_entity(origin_id) {
                    let mut dir = origin_base.pos - base.pos;
                    dir.y = 0.0; // We ignore verticale delta.
                    while dir.length_squared() < 1.0e-4 {
                        dir = DVec3 {
                            x: (base.rand.next_double() - base.rand.next_double()) * 0.01,
                            y: 0.0,
                            z: (base.rand.next_double() - base.rand.next_double()) * 0.01,
                        }
                    }
                    common::update_knock_back(base, dir);
                }
            }

        } else if hurt.damage > living.hurt_last_damage {
            actual_damage = hurt.damage - living.hurt_last_damage;
            living.hurt_last_damage = hurt.damage;
        }

        // Apply damage.
        if actual_damage != 0 {
            living.health = living.health.saturating_sub(actual_damage);
            // TODO: For players, take armor into account.
        }

    }

    if living.health == 0 {

        // If this is the first death tick, push event.
        if living.death_time == 0 {
            world.push_event(Event::Entity { id, inner: EntityEvent::Dead });
        }

        living.death_time += 1;
        if living.death_time > 20 {
            // TODO: Drop loots
            world.remove_entity(id);
        }

    }
    
}
