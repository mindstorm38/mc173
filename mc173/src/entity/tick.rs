//! Base function for ticking entity.
//! 
//! This module gives in documentation the reference to the Java methods, as known from
//! the decompilation of Minecraft b1.7.3 by RetroMCP.

use std::ops::{Add, Sub};
use std::cell::RefCell;

use glam::{DVec3, IVec3, Vec2, Vec3Swizzles};

use tracing::{trace, instrument};

use crate::world::{World, Event, EntityEvent};
use crate::block::material::Material;
use crate::util::{Face, BoundingBox};
use crate::path::PathFinder;
use crate::item::ItemStack;
use crate::block;

use super::{Entity, Size, Path,
    BaseKind, ProjectileKind, LivingKind, 
    Base, Living, 
    Item, Painting, FallingBlock, 
    Slime, LookTarget};


/// This implementation is just a wrapper to call all the inner tick functions.
impl Entity {

    /// This this entity from its id in a world.
    #[instrument(level = "debug", skip_all)]
    pub fn tick(&mut self, world: &mut World, id: u32) {
        // This function just forwards to the correct tick method.
        tick_base(world, id, &mut self.0, &mut self.1);
    }

}


// Thread local variables internally used to reduce allocation overhead.
thread_local! {
    /// Temporary entity id storage.
    static ENTITY_ID: RefCell<Vec<u32>> = const { RefCell::new(Vec::new()) };
    /// Temporary bounding boxes storage.
    static BOUNDING_BOX: RefCell<Vec<BoundingBox>> = const { RefCell::new(Vec::new()) };
    /// Temporary entity id with vector.
    static ENTITY_ID_WITH_VEC: RefCell<Vec<(u32, DVec3)>> = const { RefCell::new(Vec::new()) };
}


/// Tick base method that is common to every entity kind.
/// 
/// REF: Entity::onUpdate
fn tick_base(world: &mut World, id: u32, base: &mut Base, base_kind: &mut BaseKind) {

    // Just kill the entity if far in the void.
    if base.pos.y < -64.0 {
        world.remove_entity(id);
        return;
    }

    // If size is not coherent, get the current size and initialize the bounding box
    // from the current position.
    if !base.coherent {
        base.size = calc_size(base_kind);
        base.eye_height = calc_eye_height(base, base_kind);
        update_bounding_box_from_pos(base);
    } else if base.controlled {
        update_bounding_box_from_pos(base);
    }

    // Increase the entity lifetime, used by some entities and is interesting for debug.
    base.lifetime += 1;

    // Do not tick entity logic if the entity is externally controlled.
    if !base.controlled {
        match base_kind {
            BaseKind::Item(item) => tick_item(world, id, base, item),
            BaseKind::Painting(painting) => tick_painting(world, id, base, painting),
            BaseKind::Boat(_) => (),
            BaseKind::Minecart(_) => (),
            BaseKind::Fish(_) => (),
            BaseKind::LightningBolt(_) => (),
            BaseKind::FallingBlock(falling_block) => tick_falling_block(world, id, base, falling_block),
            BaseKind::Tnt(_) => (),
            BaseKind::Projectile(_, _) => (),
            BaseKind::Living(living, living_kind) => tick_living(world, id, base, living, living_kind),
        }
    }

    tick_base_state(world, id, base, base_kind);

    // // Only trace each second.
    // if base.lifetime % 4 == 0 && base.pos_dirty && log_enabled!(Level::Trace) {
    //     let kind = base_kind.entity_kind();
    //     let bb_size = base.bb.size();
    //     trace!("entity #{id} ({kind:?}), pos: {:.2}/{:.2}/{:.2}, bb: {:.2}/{:.2}/{:.2} -> {:.2}/{:.2}/{:.2} ({:.2}/{:.2}/{:.2})", 
    //         base.pos.x, base.pos.y, base.pos.z, 
    //         base.bb.min.x, base.bb.min.y, base.bb.min.z,
    //         base.bb.max.x, base.bb.max.y, base.bb.max.z,
    //         bb_size.x, bb_size.y, bb_size.z,
    //     );
    // }

}

/// Tick base method that is common to every entity kind, this is split in Notchian impl
/// so we split it here.
/// 
/// REF: Entity::onEntityUpdate
fn tick_base_state(world: &mut World, id: u32, base: &mut Base, base_kind: &mut BaseKind) {

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
                water_vel += calc_fluid_vel(world, pos, material, metadata);
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
        ENTITY_ID.with_borrow_mut(|picked_up_entities| {

            debug_assert!(picked_up_entities.is_empty());
            
            for (entity_id, entity, _) in world.iter_entities_colliding(base.bb.inflate(DVec3::new(1.0, 0.0, 1.0))) {

                match &entity.1 {
                    BaseKind::Item(item) => {
                        if item.frozen_time == 0 {
                            picked_up_entities.push(entity_id);
                        }
                    }
                    BaseKind::Projectile(projectile, ProjectileKind::Arrow(_)) => {
                        if projectile.block_hit.is_some() {
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

    // If this entity is living, there is more to do.
    if let BaseKind::Living(living, living_kind) = base_kind {
        tick_living_state(world, id, base, living, living_kind);
    }

}

/// REF: EntityLiving::onEntityUpdate
fn tick_living_state(world: &mut World, id: u32, base: &mut Base, living: &mut Living, living_kind: &mut LivingKind) {

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
                living.hurt_damage = living.hurt_damage.max(1);
                break;
            }

        }
    }

    // TODO: Air time underwater

    // Decrease countdowns.
    living.attack_time = living.attack_time.saturating_sub(1);
    living.hurt_time = living.hurt_time.saturating_sub(1);

    // If some damage have to be applied...
    if living.health != 0 && living.hurt_damage != 0 {

        /// The hurt time when hit for the first time.
        /// PARITY: The Notchian impl doesn't actually use hurt time but another variable
        ///  that have the exact same behavior, so we use hurt time here to be more,
        ///  consistent. We also avoid the divide by two thing that is useless.
        const HURT_INITIAL_TIME: u16 = 10;

        // Calculate the actual damage dealt on this tick depending on cooldown.
        let mut actual_damage = 0;
        if living.hurt_time == 0 {
            
            living.hurt_time = HURT_INITIAL_TIME;
            living.hurt_last_damage = living.hurt_damage;
            actual_damage = living.hurt_damage;
            world.push_event(Event::Entity { id, inner: EntityEvent::Damage });

            if let Some(origin) = living.hurt_origin {
                let mut dir = origin - base.pos;
                dir.y = 0.0; // We ignore verticle delta.
                while dir.length_squared() < 1.0e-4 {
                    dir = DVec3 {
                        x: (base.rand.next_double() - base.rand.next_double()) * 0.01,
                        y: 0.0,
                        z: (base.rand.next_double() - base.rand.next_double()) * 0.01,
                    }
                }
                update_knock_back(base, dir);
            }

        } else if living.hurt_damage > living.hurt_last_damage {
            actual_damage = living.hurt_damage - living.hurt_last_damage;
            living.hurt_last_damage = living.hurt_damage;
        }

        // Damage are reset after being applied.
        living.hurt_damage = 0;

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

/// Common method for moving an entity by a given amount while checking collisions.
/// 
/// REF: Entity::moveEntity
fn tick_base_pos(world: &mut World, _id: u32, base: &mut Base, delta: DVec3, step_height: f32) {

    if base.no_clip {
        base.bb += delta;
    } else {

        // TODO: 

        // TODO: If in cobweb:
        // delta *= DVec3::new(0.25, 0.05, 0.25)
        // base.vel = DVec3::ZERO

        // TODO: Sneaking on ground

        let colliding_bb = base.bb.expand(delta);

        // Compute a new delta that doesn't collide with above boxes.
        let mut new_delta = delta;
        
        // Use a temporarily owned thread local for colliding boxes.
        BOUNDING_BOX.with_borrow_mut(|colliding_bbs| {

            debug_assert!(colliding_bbs.is_empty());

            colliding_bbs.extend(world.iter_blocks_boxes_colliding(colliding_bb));
            colliding_bbs.extend(world.iter_entities_colliding(colliding_bb)
                .filter_map(|(_entity_id, entity, entity_bb)| {
                    // Only the boat entity acts like a hard bounding box.
                    if let Entity(_, BaseKind::Boat(_)) = entity {
                        Some(entity_bb)
                    } else {
                        None
                    }
                }));

            // Check collision on Y axis.
            for colliding_bb in &*colliding_bbs {
                new_delta.y = colliding_bb.calc_y_delta(base.bb, new_delta.y);
            }
    
            base.bb += DVec3::new(0.0, new_delta.y, 0.0);
    
            // Check collision on X axis.
            for colliding_bb in &*colliding_bbs {
                new_delta.x = colliding_bb.calc_x_delta(base.bb, new_delta.x);
            }
    
            base.bb += DVec3::new(new_delta.x, 0.0, 0.0);
    
            // Check collision on Z axis.
            for colliding_bb in &*colliding_bbs {
                new_delta.z = colliding_bb.calc_z_delta(base.bb, new_delta.z);
            }
            
            base.bb += DVec3::new(0.0, 0.0, new_delta.z);

            // Finally clear the cache.
            colliding_bbs.clear();
                
        });

        let collided_x = delta.x != new_delta.x;
        let collided_y = delta.y != new_delta.y;
        let collided_z = delta.z != new_delta.z;
        let on_ground = collided_y && delta.y < 0.0; // || self.on_ground

        // Apply step if relevant.
        if step_height > 0.0 && on_ground && (collided_x || collided_z) {
            // TODO: todo!("handle step motion");
        }

        base.on_ground = on_ground;

        if on_ground {
            if base.fall_distance > 0.0 {
                // TODO: Damage?
            }
            base.fall_distance = 0.0;
        } else if new_delta.y < 0.0 {
            base.fall_distance -= new_delta.y as f32;
        }

        if collided_x {
            base.vel.x = 0.0;
            base.vel_dirty = true;
        }

        if collided_y {
            base.vel.y = 0.0;
            base.vel_dirty = true;
        }

        if collided_z {
            base.vel.z = 0.0;
            base.vel_dirty = true;
        }

    }

    update_pos_from_bounding_box(base);

}

/// REF: EntityItem::onUpdate
fn tick_item(world: &mut World, id: u32, base: &mut Base, item: &mut Item) {

    if item.frozen_time > 0 {
        item.frozen_time -= 1;
    }

    // Update item velocity.
    base.vel_dirty = true;
    base.vel.y -= 0.04;

    // If the item is in lava, apply random motion like it's burning.
    // PARITY: The real client don't use 'in_lava', check if problematic.
    if base.in_lava {
        base.vel.y = 0.2;
        base.vel.x = ((base.rand.next_float() - base.rand.next_float()) * 0.2) as f64;
        base.vel.z = ((base.rand.next_float() - base.rand.next_float()) * 0.2) as f64;
    }

    // If the item is in an opaque block.
    let block_pos = base.pos.floor().as_ivec3();
    if world.is_block_opaque_cube(block_pos) {

        let delta = base.pos - block_pos.as_dvec3();

        // Find a block face where we can bump the item.
        let bump_face = Face::ALL.into_iter()
            .filter(|face| !world.is_block_opaque_cube(block_pos + face.delta()))
            .map(|face| {
                let mut delta = delta[face.axis_index()];
                if face.is_pos() {
                    delta = 1.0 - delta;
                }
                (face, delta)
            })
            .min_by(|&(_, delta1), &(_, delta2)| delta1.total_cmp(&delta2))
            .map(|(face, _)| face);

        // If we found a non opaque face then we bump the item to that face.
        if let Some(bump_face) = bump_face {
            let accel = (base.rand.next_float() * 0.2 + 0.1) as f64;
            if bump_face.is_neg() {
                base.vel[bump_face.axis_index()] = -accel;
            } else {
                base.vel[bump_face.axis_index()] = accel;
            }
        }
        
    }

    // Move the item while checking collisions if needed.
    tick_base_pos(world, id, base, base.vel, 0.0);

    let mut slipperiness = 0.98;

    if base.on_ground {

        slipperiness = 0.1 * 0.1 * 58.8;

        let ground_pos = IVec3 {
            x: base.pos.x.floor() as i32,
            y: base.bb.min.y.floor() as i32 - 1,
            z: base.pos.z.floor() as i32,
        };

        if let Some((ground_id, _)) = world.get_block(ground_pos) {
            if ground_id != block::AIR {
                slipperiness = block::material::get_slipperiness(ground_id);
            }
        }

    }

    // Slow its velocity depending on ground slipperiness.
    base.vel.x *= slipperiness as f64;
    base.vel.y *= 0.98;
    base.vel.z *= slipperiness as f64;
    
    if base.on_ground {
        base.vel.y *= -0.5;
    }

    // Kill the item self after 5 minutes (5 * 60 * 20).
    if base.lifetime >= 6000 {
        world.remove_entity(id);
    }

}

/// REF: EntityPainting::onUpdate
fn tick_painting(_world: &mut World, _id: u32, _base: &mut Base, painting: &mut Painting) {
    painting.check_valid_time += 1;
    if painting.check_valid_time >= 100 {
        painting.check_valid_time = 0;
        // TODO: check painting validity and destroy it if not valid
    }
}

/// REF: EntityFallingSand::onUpdate
fn tick_falling_block(world: &mut World, id: u32, base: &mut Base, falling_block: &mut FallingBlock) {

    if falling_block.block_id == 0 {
        world.remove_entity(id);
        return;
    }

    base.vel_dirty = true;
    base.vel.y -= 0.04;

    tick_base_pos(world, id, base, base.vel, 0.0);

    if base.on_ground {

        base.vel *= DVec3::new(0.7, -0.5, 0.7);
        world.remove_entity(id);

        let block_pos = base.pos.floor().as_ivec3();
        if world.can_place_block(block_pos, Face::PosY, falling_block.block_id) {
            world.set_block_notify(block_pos, falling_block.block_id, 0);
        } else {
            world.spawn_loot(base.pos, ItemStack::new_block(falling_block.block_id, 0), 0.0);
        }

    } else if base.lifetime > 100 {
        world.remove_entity(id);
        world.spawn_loot(base.pos, ItemStack::new_block(falling_block.block_id, 0), 0.0);
    }

}

/// REF: EntityLiving::onUpdate
fn tick_living(world: &mut World, id: u32, base: &mut Base, living: &mut Living, living_kind: &mut LivingKind) {

    tick_living_ai(world, id, base, living, living_kind);

    if living.jumping {
        if base.in_water || base.in_lava {
            base.vel_dirty = true;
            base.vel.y += 0.04;
        } else if base.on_ground {
            base.vel_dirty = true;
            base.vel.y += 0.42 + 0.1; // FIXME: Added 0.1 to make it work
        }
    }

    living.accel_strafing *= 0.98;
    living.accel_forward *= 0.98;
    living.yaw_velocity *= 0.9;

    tick_living_pos(world, id, base, living, living_kind);
    tick_living_push(world, id, base);

}

/// REF: EntityLiving::updatePlayerActionState
fn tick_living_ai(world: &mut World, id: u32, base: &mut Base, living: &mut Living, living_kind: &mut LivingKind) {

    /// Multiplier for random yaw velocity: 20 deg
    const YAW_VELOCITY_MUL: f32 = 0.3490658503988659;
    /// Maximum distance for looking at a target.
    const LOOK_AT_MAX_DIST: f64 = 8.0;
    /// Default look step when looking at a target.
    const LOOK_STEP: Vec2 = Vec2::new(0.17453292519943295, 0.6981317007977318);
    /// Slow look step used for sitting dogs.
    const SLOW_LOOK_STEP: Vec2 = Vec2::new(0.17453292519943295, 0.3490658503988659);

    let handled = match living_kind {
        LivingKind::Human(_) => true,  // For now we do nothing.
        LivingKind::Ghast(_) => true,
        LivingKind::Squid(_) => true,
        LivingKind::Slime(slime) => tick_slime_ai(world, id, base, living, slime),
        // All other unmatched entity kinds are ground creature.
        _ => tick_ground_ai(world, id, base, living, living_kind),
    };

    // If the entity-specific AI has been handled, do not run the default AI.
    if handled {
        return;
    }

    // TODO: Handle kill when closest player is too far away.

    living.accel_strafing = 0.0;
    living.accel_forward = 0.0;

    if base.rand.next_float() < 0.02 {
        if let Some((target_entity_id, _)) = find_closest_player_entity(world, base.pos, LOOK_AT_MAX_DIST) {
            living.look_target = Some(LookTarget {
                entity_id: target_entity_id,
                remaining_time: base.rand.next_int_bounded(20) as u32 + 10,
            });
        } else {
            living.yaw_velocity = (base.rand.next_float() - 0.5) * YAW_VELOCITY_MUL;
        }
    }

    // If the entity should have a target, just look at it if possible, and stop if
    // the target should end or is too far away.
    if let Some(target) = &mut living.look_target {

        target.remaining_time = target.remaining_time.saturating_sub(1);
        let mut target_release = target.remaining_time == 0;

        if let Some(Entity(target_base, _)) = world.get_entity(target.entity_id) {
            
            let mut look_step = LOOK_STEP;
            if let LivingKind::Wolf(wolf) = living_kind {
                if wolf.sitting {
                    look_step = SLOW_LOOK_STEP;
                }
            }

            update_look_at_entity_by_step(base, target_base, look_step);
            
            if target_base.pos.distance_squared(base.pos) > LOOK_AT_MAX_DIST.powi(2) {
                target_release = false;
            }

        } else {
            // Entity is dead.
            target_release = false;
        }

        if target_release {
            living.look_target = None;
        }

    } else {

        if base.rand.next_float() < 0.05 {
            living.yaw_velocity = (base.rand.next_float() - 0.5) * YAW_VELOCITY_MUL;
        }

        base.look.x += living.yaw_velocity;
        base.look.y = 0.0;
        base.look_dirty = true;

    }

    if base.in_water || base.in_lava {
        living.jumping = base.rand.next_float() < 0.8;
    }

}

/// Tick an ground creature (animal/mob) entity AI.
/// 
/// REF: EntityCreature::updatePlayerActionState
fn tick_ground_ai(world: &mut World, id: u32, base: &mut Base, living: &mut Living, living_kind: &mut LivingKind) -> bool {

    /// Maximum distance for the path finder.
    const PATH_FINDER_MAX_DIST: f32 = 16.0;
    /// Look step when looking at an attacked entity: 30/30 deg
    const LOOK_STEP: Vec2 = Vec2::new(0.5235987755982988, 0.5235987755982988);

    let mut has_attacked = false;
    // Keep the entity reference to avoid querying too much the world.
    let mut attack_target_base = None;

    // Start by finding an attack target, or attack the existing one.
    if let Some(target_id) = living.attack_target {

        if let Some(Entity(target_base, _)) = world.get_entity(target_id) {
            attack_target_base = Some(target_base);
            // TODO: Seen: attack, not seen: release attack (creeper)
            // TODO: has_attacked = true
        } else {
            // Entity is no longer existing, target lost.
            trace!("entity #{id}, attack target is dead");
            living.attack_target = None;
        }

    } else  {
        
        // Depending on the entity, we search an attack target or not...
        let search_around = match living_kind {
            LivingKind::Creeper(_) => true,
            LivingKind::Giant(_) => true,
            LivingKind::Skeleton(_) => true,
            LivingKind::Zombie(_) => true,
            LivingKind::PigZombie(pig_zombie) => pig_zombie.anger,
            LivingKind::Wolf(wolf) => wolf.angry,
            LivingKind::Spider(_) => calc_entity_brightness(world, base) < 0.5,
            _ => false,
        };

        if search_around {
            if let Some((target_id, Entity(target_base, _))) = find_closest_player_entity(world, base.pos, 16.0) {
                trace!("entity #{id}, found entity #{target_id} to attack");
                attack_target_base = Some(target_base);
                living.attack_target = Some(target_id);
                living.path = PathFinder::new(world).find_path_from_bounding_box(base.bb, target_base.pos, PATH_FINDER_MAX_DIST).map(Path::from);
            }
        }

    }

    // If the entity has not attacked its target entity and is path finder toward it, 
    // there is 95% chance too go into the then branch.
    if has_attacked || attack_target_base.is_none() || (living.path.is_some() && base.rand.next_int_bounded(20) != 0) {
        // If the entity has not attacked and if the path is not none, there is 1.25% 
        // chance to recompute the path, if the path is none there is 2.484375% chance.
        if !has_attacked && ((living.path.is_none() && base.rand.next_int_bounded(80) == 0) || base.rand.next_int_bounded(80) == 0) {

            // The path weight function depends on the entity type.
            let weight_func = match living_kind {
                LivingKind::Pig(_) |
                LivingKind::Chicken(_) |
                LivingKind::Cow(_) |
                LivingKind::Sheep(_) |
                LivingKind::Wolf(_) => path_weight_animal,
                LivingKind::Creeper(_) |
                LivingKind::PigZombie(_) |
                LivingKind::Skeleton(_) |
                LivingKind::Spider(_) |
                LivingKind::Zombie(_) => path_weight_mob,
                LivingKind::Giant(_) => path_weight_giant,
                // We should not match other entities but we never known...
                _ => path_weight_default,
            };
            
            let best_pos = (0..10)
                .map(|_| {
                    IVec3 {
                        x: base.pos.x.add((base.rand.next_int_bounded(13) - 6) as f64).floor() as i32,
                        y: base.pos.y.add((base.rand.next_int_bounded(7) - 3) as f64).floor() as i32,
                        z: base.pos.z.add((base.rand.next_int_bounded(13) - 6) as f64).floor() as i32,
                    }
                })
                .map(|pos| (pos, weight_func(world, pos)))
                .max_by(|(_, a), (_, b)| a.total_cmp(b))
                .unwrap().0;

            trace!("entity #{id}, path finding: {best_pos}");

            let best_pos = best_pos.as_dvec3() + 0.5;
            if let Some(points) = PathFinder::new(world).find_path_from_bounding_box(base.bb, best_pos, PATH_FINDER_MAX_DIST) {
                trace!("entity #{id}, path found");
                living.path = Some(Path::from(points));
            }
            
        }
    } else {
        // NOTE: Here we are guaranteed that attack target is present.
        let target_base = attack_target_base.unwrap();
        living.path = PathFinder::new(world).find_path_from_bounding_box(base.bb, target_base.pos, PATH_FINDER_MAX_DIST).map(Path::from);
    }

    if let Some(path) = &mut living.path {

        // // Debug particles, lava = remaining, water = done.
        // if base.lifetime % 10 == 0 {
        //     for (i, pos) in path.points.iter().copied().enumerate() {
        //         if i < path.index {
        //             world.push_event(Event::DebugParticle { pos, block: block::WATER_STILL });
        //         } else {
        //             world.push_event(Event::DebugParticle { pos, block: block::LAVA_STILL });
        //         }
        //     }
        // }

        if base.rand.next_int_bounded(100) != 0 {

            let bb_size = base.bb.size();
            let double_width = bb_size.x * 2.0;

            let mut next_pos = None;
            
            while let Some(pos) = path.point() {

                let mut pos = pos.as_dvec3();
                pos.x += (bb_size.x + 1.0) * 0.5;
                pos.z += (bb_size.z + 1.0) * 0.5;

                // Advance the path to the next point only if distance to current one is 
                // too short. We only check the horizontal distance, because Y delta is 0.
                let pos_dist_sq = pos.distance_squared(DVec3::new(base.pos.x, pos.y, base.pos.z));
                if pos_dist_sq < double_width * double_width {
                    trace!("entity #{id}, path pos to short: {pos}, dist: {} < {}", pos_dist_sq.sqrt(), double_width);
                    path.advance();
                } else {
                    next_pos = Some(pos);
                    break;
                }

            }

            living.jumping = false;

            if let Some(next_pos) = next_pos {

                trace!("entity #{id}, path next pos: {next_pos}");

                let dx = next_pos.x - base.pos.x;
                let dy = next_pos.y - base.bb.min.y.add(0.5).floor();
                let dz = next_pos.z - base.pos.z;

                let move_speed = match living_kind {
                    LivingKind::Giant(_) |
                    LivingKind::Zombie(_) |
                    LivingKind::PigZombie(_) => 0.5,
                    LivingKind::Spider(_) => 0.8,
                    _ => 0.5,
                };

                living.accel_forward = move_speed;
                base.look.x = f64::atan2(dz, dx) as f32 - std::f32::consts::FRAC_PI_2;
                base.look_dirty = true;

                // Make some weird strafing if we just attacked the player.
                if has_attacked {
                    if let Some(target_base) = attack_target_base {
                        let dx = target_base.pos.x - base.pos.x;
                        let dz = target_base.pos.z - base.pos.z;
                        base.look.x = f64::atan2(dz, dx) as f32 - std::f32::consts::FRAC_PI_2;
                        living.accel_strafing = -base.look.x.sin() * living.accel_forward;
                        living.accel_forward = base.look.x.cos() * living.accel_forward;
                    }
                }

                if dy > 0.0 {
                    living.jumping = true;
                }

            } else {
                trace!("entity #{id}, path finished");
                living.path = None;
            }

            // Look at the player we are attacking.
            if let Some(target_base) = attack_target_base {
                update_look_at_entity_by_step(base, target_base, LOOK_STEP);
            }

            // TODO: If collided horizontal and no path, then jump

            if base.rand.next_float() < 0.8 && (base.in_water || base.in_lava) {
                trace!("entity #{id}, jumping because of 80% chance or water/lava");
                living.jumping = true;
            }

            // Return true because the AI is handled by this function.
            return true;

        } else {
            trace!("entity #{id}, forget path because 1% chance")
        }

    }

    // If we can't run a path finding AI, fallback to the default immobile AI.
    living.path = None;
    false

}

/// Tick a slime entity AI.
/// 
/// REF: EntitySlime::updatePlayerActionState
fn tick_slime_ai(world: &mut World, _id: u32, base: &mut Base, living: &mut Living, slime: &mut Slime) -> bool {

    /// Look step for slime: 10/20 deg
    const LOOK_STEP: Vec2 = Vec2::new(0.17453292519943295, 0.3490658503988659);

    // TODO: despawn entity if too far away from player

    // Searching the closest player entities behind 16.0 blocks.
    let closest_player = find_closest_player_entity(world, base.pos, 16.0);
    if let Some((_, Entity(closest_base, _))) = closest_player {
        update_look_at_entity_by_step(base, closest_base, LOOK_STEP);
    }

    let mut set_jumping = false;
    if base.on_ground {
        slime.jump_remaining_time = slime.jump_remaining_time.saturating_sub(1);
        if slime.jump_remaining_time == 0 {
            set_jumping = true;
        }
    }

    if set_jumping {
        
        slime.jump_remaining_time = base.rand.next_int_bounded(20) as u32 + 10;

        if closest_player.is_some() {
            slime.jump_remaining_time /= 3;
        }

        living.jumping = true;
        living.accel_strafing = 1.0 - base.rand.next_float() * 2.0;
        living.accel_forward = slime.size as f32;

    } else {
        living.jumping = false;
        if base.on_ground {
            living.accel_strafing = 0.0;
            living.accel_forward = 0.0;
        }
    }

    true

}

/// Tick a living entity to push/being pushed an entity.
fn tick_living_push(world: &mut World, _id: u32, base: &mut Base) {

    // TODO: pushing minecart

    // Temporarily owned vector in order to compute pushed entities.
    // REF: Entity::applyEntityCollision
    ENTITY_ID_WITH_VEC.with_borrow_mut(|pushed_entities| {

        debug_assert!(pushed_entities.is_empty());

        // For each colliding entity, precalculate the velocity to add to both entities.
        for (push_id, push_entity, _) in world.iter_entities_colliding(base.bb.inflate(DVec3::new(0.2, 0.0, 0.2))) {
            
            let Entity(push_base, push_base_kind) = push_entity;

            match push_base_kind {
                BaseKind::Boat(_) |
                BaseKind::Living(_, _) |
                BaseKind::Minecart(_) => {}
                _ => continue // Other entities cannot be pushed.
            }

            let mut dx = base.pos.x - push_base.pos.x;
            let mut dz = base.pos.z - push_base.pos.z;
            let mut delta = f64::max(dx.abs(), dz.abs());
            
            if delta >= 0.01 {
                
                delta = delta.sqrt();
                dx /= delta;
                dz /= delta;

                let delta_inv = 1.0 / delta;
                dx *= delta_inv;
                dz *= delta_inv;
                dx *= 0.05;
                dz *= 0.05;

                pushed_entities.push((push_id, DVec3::new(dx, 0.0, dz)));

            }

        }

        for (push_id, push_delta) in pushed_entities.drain(..) {
            let Entity(push_base, _) = world.get_entity_mut(push_id).unwrap();
            push_base.vel -= push_delta;
            base.vel += push_delta;
            push_base.vel_dirty = true;
            base.vel_dirty = true;
        }

    });

}

/// REF: EntityLiving::moveEntityWithHeading
fn tick_living_pos(world: &mut World, id: u32, base: &mut Base, living: &mut Living, living_kind: &mut LivingKind) {

    // Squid has no special rule for moving.
    if let LivingKind::Squid(_) = living_kind {
        tick_base_pos(world, id, base, base.vel, 0.5);
        return;
    }

    // All living entities have step height 0.5;
    let step_height = 0.5;

    // REF: EntityFlying::moveEntityWithHeading
    let flying = matches!(living_kind, LivingKind::Ghast(_));

    if base.in_water {
        tick_living_vel(world, id, base, living, 0.02);
        tick_base_pos(world, id, base, base.vel, step_height);
        base.vel *= 0.8;
        if !flying {
            base.vel.y -= 0.02;
        }
        // TODO: If collided horizontally
    } else if base.in_lava {
        tick_living_vel(world, id, base, living, 0.02);
        tick_base_pos(world, id, base, base.vel, step_height);
        base.vel *= 0.5;
        if !flying {
            base.vel.y -= 0.02;
        }
        // TODO: If collided horizontally
    } else {

        let mut slipperiness = 0.91;

        if base.on_ground {
            slipperiness = 546.0 * 0.1 * 0.1 * 0.1;
            let ground_pos = base.pos.as_ivec3();
            if let Some((ground_id, _)) = world.get_block(ground_pos) {
                if ground_id != 0 {
                    slipperiness = block::material::get_slipperiness(ground_id) * 0.91;
                }
            }
        }

        // Change entity velocity if on ground or not.
        let vel_factor = match base.on_ground {
            true => 0.16277136 / (slipperiness * slipperiness * slipperiness) * 0.1,
            false => 0.02,
        };

        tick_living_vel(world, id, base, living, vel_factor);
        
        // TODO: Is on ladder

        tick_base_pos(world, id, base, base.vel, step_height);

        // TODO: Collided horizontally and on ladder

        if flying {
            base.vel *= slipperiness as f64;
        } else {
            base.vel.y -= 0.08;
            base.vel.y *= 0.98;
            base.vel.x *= slipperiness as f64;
            base.vel.z *= slipperiness as f64;
        }

    }

    base.vel_dirty = true;
    
}

/// Update a living entity velocity according to its strafing/forward accel.
fn tick_living_vel(_world: &mut World, _id: u32, base: &mut Base, living: &mut Living, factor: f32) {

    let mut strafing = living.accel_strafing;
    let mut forward = living.accel_forward;
    let mut dist = Vec2::new(forward, strafing).length();
    if dist >= 0.01 {
        dist = dist.max(1.0);
        dist = factor / dist;
        strafing *= dist;
        forward *= dist;
        let (yaw_sin, yaw_cos) = base.look.x.sin_cos();
        base.vel_dirty = true;
        base.vel.x += (strafing * yaw_cos - forward * yaw_sin) as f64;
        base.vel.z += (forward * yaw_cos + strafing * yaw_sin) as f64;
    }
    
}

// =========================== //
// Below are utility functions //
// =========================== //

/// Calculate the initial size of an entity, this is only called when not coherent.
fn calc_size(base_kind: &mut BaseKind) -> Size {
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
        BaseKind::Projectile(_, ProjectileKind::Egg(_)) =>Size::new(0.5, 0.5),
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
fn calc_eye_height(base: &Base, base_kind: &BaseKind) -> f32 {
    match base_kind {
        BaseKind::Living(_, LivingKind::Human(_)) => 1.62,
        BaseKind::Living(_, LivingKind::Wolf(_)) => base.size.height * 0.8,
        BaseKind::Living(_, _) => base.size.height * 0.85,
        _ => 0.0,
    }
}

/// Calculate the velocity of a fluid at given position, this depends on neighbor blocks.
/// This calculation will only take the given material into account, this material should
/// be a fluid material (water/lava), and the given metadata should be the one of the
/// current block the the position.
fn calc_fluid_vel(world: &World, pos: IVec3, material: Material, metadata: u8) -> DVec3 {

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

fn calc_entity_brightness(world: &World, base: &Base) -> f32 {
    let mut check_pos = base.pos;
    check_pos.y += base.size.height as f64 * 0.66;
    world.get_brightness(check_pos.floor().as_ivec3()).unwrap_or(0.0)
}

fn find_closest_player_entity(world: &World, center: DVec3, dist: f64) -> Option<(u32, &Entity)> {
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
fn update_bounding_box_from_pos(base: &mut Base) {
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
fn update_pos_from_bounding_box(base: &mut Base) {
    
    let center = base.size.center as f64;
    let new_pos = DVec3 {
        x: (base.bb.min.x + base.bb.max.x) / 2.0,
        y: base.bb.min.y + center,
        z: (base.bb.min.z + base.bb.max.z) / 2.0,
    };

    if new_pos != base.pos {
        base.pos = new_pos;
        base.pos_dirty = true;
    }
    
}

/// Modify the look angles of this entity, limited to the given step. 
/// We need to call this function many time to reach the desired look.
fn update_look_by_step(base: &mut Base, look: Vec2, step: Vec2) {
    
    let look_norm = Vec2 {
        // Yaw can be normalized between 0 and tau
        x: look.x.rem_euclid(std::f32::consts::TAU),
        // Pitch however needs to be normalized between -pi/2 and pi/2
        y: look.y,
    };

    let delta = look_norm.sub(base.look).min(step);
    if delta != Vec2::ZERO {
        base.look_dirty = true;
        base.look += delta;
    }

}

/// Modify the look angles to point to a given target step by step. The eye height is
/// included in the calculation in order to make the head looking at target.
fn update_look_at_by_step(base: &mut Base, target: DVec3, step: Vec2) {
    
    let mut delta = target - base.pos;
    delta.y -= base.eye_height as f64;

    let horizontal_dist = delta.xz().length();
    let yaw = f64::atan2(delta.z, delta.x) as f32 - std::f32::consts::FRAC_PI_2;
    let pitch = -f64::atan2(delta.y, horizontal_dist) as f32;
    update_look_by_step(base, Vec2::new(yaw, pitch), step);

}

/// Almost the same as [`update_look_at_by_step`] but the target is another entity base,
/// this function will make the entity look at the eyes of the target one.
fn update_look_at_entity_by_step(base: &mut Base, target_base: &Base, step: Vec2) {
    let mut target = target_base.pos;
    target.y += target_base.eye_height as f64;
    update_look_at_by_step(base, target, step);
}

/// Apply knock back to this entity's velocity.
fn update_knock_back(base: &mut Base, dir: DVec3) {

    let mut accel = dir.normalize_or_zero();
    accel.y -= 1.0;

    base.vel_dirty = true;
    base.vel /= 2.0;
    base.vel -= accel * 0.4;
    base.vel.y = base.vel.y.min(0.4);

}

/// Path weight function for animals.
fn path_weight_animal(world: &World, pos: IVec3) -> f32 {
    if world.is_block(pos - IVec3::Y, block::GRASS) {
        10.0
    } else {
        world.get_brightness(pos).unwrap_or(0.0) - 0.5
    }
}

/// Path weight function for mobs.
fn path_weight_mob(world: &World, pos: IVec3) -> f32 {
    0.5 - world.get_brightness(pos).unwrap_or(0.0)
}

/// Path weight function for Giant.
fn path_weight_giant(world: &World, pos: IVec3) -> f32 {
    world.get_brightness(pos).unwrap_or(0.0) - 0.5
}

/// Path weight function by default.
fn path_weight_default(_world: &World, _pos: IVec3) -> f32 {
    0.0
}
