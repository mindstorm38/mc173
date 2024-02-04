use bevy::ecs::system::{Commands, Query};
use bevy::ecs::entity::Entity;
use bevy::math::DVec3;

use super::{Base, Item, Real};


/// Tick all entity lifetime.
pub fn tick_all(
    mut commands: Commands,
    mut query: Query<(Entity, &mut Base, Option<&Real>)>
) {

    for (entity, mut base, real) in query.iter_mut() {
        base.lifetime += 1;
        if let Some(real) = real {
            if real.pos.y < -64.0 {
                commands.entity(entity).despawn();
            }
        }
    }

}

/// Common tick state for entities.
pub fn tick_state_real(
    mut query: Query<(Entity, &mut Real, Option<&Item>)>
) {

    for (
        entity,
        mut real,
        mut item,
    ) in query.iter_mut() {

        // Compute the bounding box used for water collision, items are different.
        let water_bb = match item {
            Some(_) => real.bb,
            None => real.bb.inflate(DVec3::new(-0.001, -0.4 - 0.001, -0.001)),
        };

        // Search for water block in the water bb.
        real.in_water = false;
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
            real.vel += water_vel * 0.014;
        }
        
    }
    
}

pub fn tick_item(
    mut query: Query<(&mut Base, &mut Real, &mut Item)>
) {

    for (
        mut base, 
        mut real, 
        mut item
    ) in query.iter_mut() {

        if item.frozen_time > 0 {
            item.frozen_time -= 1;
        }

        // Update item velocity.
        real.vel.y -= 0.04;
    
        // If the item is in lava, apply random motion like it's burning.
        // PARITY: The real client don't use 'in_lava', check if problematic.
        if real.in_lava {
            real.vel.y = 0.2;
            real.vel.x = ((base.rand.next_float() - base.rand.next_float()) * 0.2) as f64;
            real.vel.z = ((base.rand.next_float() - base.rand.next_float()) * 0.2) as f64;
        }

    }

}
