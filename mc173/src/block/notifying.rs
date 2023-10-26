//! Handles block changes notifications.

use std::collections::HashMap;

use glam::IVec3;

use crate::util::Face;
use crate::world::World;
use crate::block;


/// Notify all blocks around the given block of a block change.
pub fn notify_around(world: &mut World, pos: IVec3) {
    notify_at(world, pos - IVec3::X);
    notify_at(world, pos + IVec3::X);
    notify_at(world, pos - IVec3::Y);
    notify_at(world, pos + IVec3::Y);
    notify_at(world, pos - IVec3::Z);
    notify_at(world, pos + IVec3::Z);
}

/// Notify a block at some position that a neighbor block has changed.
pub fn notify_at(world: &mut World, pos: IVec3) {

    let Some((id, metadata)) = world.block_and_metadata(pos) else { return };

    match id {
        block::REDSTONE => notify_redstone(world, pos),
        block::REPEATER |
        block::REPEATER_LIT => notify_repeater(world, pos, id, metadata),
        _ => {}
    }

}

/// Notification of a redstone wire block.
fn notify_redstone(world: &mut World, pos: IVec3) {

    // TODO: Drop if the block can no longer be placed at its position.

    const FACES: [Face; 4] = [Face::NegX, Face::PosX, Face::NegZ, Face::PosZ];

    let mut nodes = HashMap::new();
    let mut pending = Vec::new();
    let mut nodes_power = Vec::new();

    nodes.insert(pos, 0);
    pending.push((pos, Face::PosY));
    
    while let Some((pending_pos, parent_face)) = pending.pop() {
        for face in FACES {
            // NOTE: We know that parent face is also a redstone dust.
            if face != parent_face {
                let face_pos = pending_pos + face.delta();
                if let Some((id, _)) = world.block_and_metadata(face_pos) {
                    if id == block::REDSTONE {
                        if nodes.insert(face_pos, 0u8).is_none() {
                            pending.push((face_pos, face.opposite()));
                        }
                    } else {
                        let face_power = block::powering::get_direct_power_from(world, face_pos, face.opposite());
                        if face_power > 0 {
                            nodes_power.push((pending_pos, face_power));
                        }
                    }
                }
            }
        }
    }

    while !nodes_power.is_empty() {

        for (node_pos, min_power) in nodes_power.drain(..) {
            let power = nodes.get_mut(&node_pos).unwrap();
            *power = min_power.max(*power);
            pending.push((node_pos, Face::PosY));
        }

        for (node_pos, _) in pending.drain(..) {
            if let Some(power) = nodes.remove(&node_pos) {

                world.set_block_and_metadata(node_pos, block::REDSTONE, power);

                // Only update neighbor nodes if power is greater than 1, because 1 would
                // give a power of 0 when propagated, and this is handled by the end loop.
                if power > 2 {
                    for face in FACES {
                        let face_pos = node_pos + face.delta();
                        // Check if there is an existing node on that face.
                        if let Some(face_power) = nodes.get(&face_pos).copied() {
                            // This node has no power yet.
                            if face_power == 0u8 {
                                // Decrease power by one.
                                nodes_power.push((face_pos, power - 1));
                            }
                        }
                    }
                }

            }
        }

    }

    // When there are no remaining power to apply, just set all remaining nodes to off.
    for node_pos in nodes.into_keys() {
        world.set_block_and_metadata(node_pos, block::REDSTONE, 0);
    }

}

/// Notification of a redstone repeater block.
fn notify_repeater(world: &mut World, pos: IVec3, id: u8, metadata: u8) {

    let lit = id == block::REPEATER_LIT;
    let face = block::repeater::get_face(metadata);
    let delay = block::repeater::get_delay_ticks(metadata);
    let back_powered = block::powering::get_passive_power_from(world, pos - face.delta(), face) != 0;

    if lit != back_powered {
        world.schedule_tick(pos, id, delay);
    }

}
