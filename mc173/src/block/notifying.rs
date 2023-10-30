//! Handles block changes notifications.

use std::collections::hash_map::Entry;
use std::collections::{HashMap, HashSet};

use glam::IVec3;

use crate::util::{Face, FaceSet};
use crate::world::World;
use crate::block;


/// Notify all blocks around the given block of a block change.
pub fn notify_around(world: &mut World, pos: IVec3) {
    notify_at(world, pos - IVec3::X, true);
    notify_at(world, pos + IVec3::X, true);
    notify_at(world, pos - IVec3::Y, true);
    notify_at(world, pos + IVec3::Y, true);
    notify_at(world, pos - IVec3::Z, true);
    notify_at(world, pos + IVec3::Z, true);
}

/// Notify of some block change in the world, this is typically called from the world's
/// `set_block_self_notify` method when block is changed.
pub fn changed_at(world: &mut World, pos: IVec3, prev_id: u8, prev_metadata: u8, id: u8, metadata: u8) {

    match prev_id {
        block::BUTTON => {
            if let Some(face) = block::button::get_face(prev_metadata) {
                notify_around(world, pos + face.delta());
            }
        }
        block::LEVER => {
            if let Some((face, _)) = block::lever::get_face(prev_metadata) {
                notify_around(world, pos + face.delta());
            }
        }
        _ => {}
    }

    if prev_id != id {
        match (prev_id, id) {
            (block::REDSTONE_TORCH, block::REDSTONE_TORCH_LIT) |
            (block::REDSTONE_TORCH_LIT, block::REDSTONE_TORCH) => {
                notify_around(world, pos + IVec3::Y);
            }
            (block::REPEATER, block::REPEATER_LIT) |
            (block::REPEATER_LIT, block::REPEATER) => {
                notify_around(world, pos + block::repeater::get_face(metadata).delta());
            }
            (_, block::REDSTONE) => notify_redstone(world, pos),
            (_, block::REDSTONE_TORCH | 
                block::REDSTONE_TORCH_LIT) => notify_redstone_torch(world, pos, id),
            (_, block::REPEATER |
                block::REPEATER_LIT) => notify_repeater(world, pos, id, metadata),
            _ => {}
        }
    }

}

/// Internal function to notify a block at the given position, this also provides control
/// over redstone notification in order to avoid infinite recursion in the redstone 
/// update logic.
fn notify_at(world: &mut World, pos: IVec3, redstone: bool) {

    let Some((id, metadata)) = world.block(pos) else { return };
    
    match id {
        block::REDSTONE if redstone => notify_redstone(world, pos),
        block::REPEATER |
        block::REPEATER_LIT => notify_repeater(world, pos, id, metadata),
        block::REDSTONE_TORCH |
        block::REDSTONE_TORCH_LIT => notify_redstone_torch(world, pos, id),
        block::WATER_MOVING |
        block::LAVA_MOVING => notify_fluid_moving(world, pos, id),
        block::WATER_STILL |
        block::LAVA_STILL => notify_fluid_still(world, pos, id, metadata),
        _ => {}
    }

}

/// Notification of a redstone wire block.
fn notify_redstone(world: &mut World, pos: IVec3) {

    const FACES: [Face; 4] = [Face::NegX, Face::PosX, Face::NegZ, Face::PosZ];

    /// Internal structure to keep track of the power and links of a single redstone.
    #[derive(Default)]
    struct Node {
        /// The current power of this node.
        power: u8,
        /// This bit fields contains, for each face of the redstone node, if it's linked
        /// to another redstone, that may be on top or bottom or the faced block. So this
        /// is not an exact indication but rather a hint.
        links: FaceSet,
        /// True when there is an opaque block above the node, so it could spread above.
        opaque_above: bool,
        /// True when there is an opaque block below the node, so it could spread below.
        opaque_below: bool,
    }

    // TODO: Use thread-local allocated maps and vectors...

    // Nodes mapped to their position.
    let mut nodes: HashMap<IVec3, Node> = HashMap::new();
    // Queue of nodes pending to check their neighbor blocks, each pending node is 
    // associated to a face leading to the node that added it to the list.
    let mut pending: Vec<(IVec3, Face)> = vec![(pos, Face::NegY)];
    // Queue of nodes that should propagate their power on the next propagation loop.
    // The associated boolean is used when propagating sources to indicate if the power
    // has changed from its previous value.
    let mut sources: Vec<IVec3> = Vec::new();

    // This loop constructs the network on nodes and give the initial external power to
    // nodes that are connected to a source.
    while let Some((pending_pos, link_face)) = pending.pop() {

        let node = match nodes.entry(pending_pos) {
            Entry::Occupied(o) => {
                // If our pending node is already existing, just update the link to it.
                o.into_mut().links.insert(link_face);
                // Each node is checked for sources once, so we continue.
                continue;
            }
            Entry::Vacant(v) => {
                v.insert(Node::default())
            }
        };

        // Linked to the block that discovered this pending node.
        node.links.insert(link_face);

        // Check if there is an opaque block above, used to prevent connecting top nodes.
        node.opaque_above = world.block(pos + IVec3::Y)
            .map(|(above_id, _)| block::material::is_opaque_cube(above_id))
            .unwrap_or(true);
        node.opaque_below = world.block(pos - IVec3::Y)
            .map(|(below_id, _)| block::material::is_opaque_cube(below_id))
            .unwrap_or(true);

        for face in FACES {

            // Do not process the face that discovered this node: this avoid too many
            // recursion, and this is valid since 
            if link_face == face {
                continue;
            }

            let face_pos = pending_pos + face.delta();
            if let Some((id, _)) = world.block(face_pos) {

                if id == block::REDSTONE {
                    node.links.insert(face);
                    pending.push((face_pos, face.opposite()));
                    continue;
                }

                // If the faced block is not a redstone, get the direct power from it and
                // update our node initial power depending on it.
                let face_power = block::powering::get_active_power_from(world, face_pos, face.opposite());
                node.power = node.power.max(face_power);

                if block::from_id(id).material.is_opaque() {
                    // If that faced block is opaque, we check if a redstone dust is 
                    // present on top of it, we connect the network to it if not opaque 
                    // above.
                    if !node.opaque_above {
                        let face_above_pos = face_pos + IVec3::Y;
                        if let Some((block::REDSTONE, _)) = world.block(face_above_pos) {
                            node.links.insert(face);
                            pending.push((face_above_pos, face.opposite()));
                        }
                    }
                } else {
                    // If the faced block is not opaque, the power can come from below
                    // the faced block, so we connect if this is redstone.
                    // NOTE: If the block below is not opaque, the signal cannot come to
                    // the current node, but that will be resolved in the loop below.
                    let face_below_pos = face_pos - IVec3::Y;
                    if let Some((block::REDSTONE, _)) = world.block(face_below_pos) {
                        node.links.insert(face);
                        pending.push((face_below_pos, face.opposite()));
                    }
                }

            }

        }

        // Check above and below for pure power sources, do not check if this is redstone
        // as it should not be possible to place, theoretically.
        for face in [Face::NegY, Face::PosY] {
            let face_pos = pending_pos + face.delta();
            let face_power = block::powering::get_active_power_from(world, face_pos, face.opposite());
            node.power = node.power.max(face_power);
        }

        if node.power > 0 {
            sources.push(pending_pos);
        }

    }

    // No longer used, just as a note.
    drop(pending);

    // The index of the first next source to propagate. At the end of the algorithm, the
    // whole sources vector will be filled will all nodes in descending order by distance
    // to nearest source.
    let mut next_sources_index = 0;

    // A list of nodes that changes their power value after update. They are naturally
    // ordered from closest to source to farthest. Every node should be present once.
    let mut changed_nodes = Vec::new();

    // While sources are remaining to propagate.
    while next_sources_index < sources.len() {

        // Iterate from next sources index to the current length of the vector (excluded)
        // while updating the next sources index to point to that end. So all added 
        // sources will be placed after that index and processed on next loop.
        let start_index = next_sources_index;
        let end_index = sources.len();
        next_sources_index = end_index;

        for source_index in start_index..end_index {

            let node_pos = sources[source_index];

            // Pop the node and finally update its block power. Ignore if the node have
            // already been processed.
            let Some(node) = nodes.remove(&node_pos) else { continue };

            // Set block and update the changed boolean of that source.
            if world.set_block(node_pos, block::REDSTONE, node.power) != Some((block::REDSTONE, node.power)) {
                changed_nodes.push(node_pos);
            }

            // If the power is one or below (should not happen), do not process face 
            // because the power will be out anyway.
            if node.power <= 1 {
                continue;
            }

            let propagated_power = node.power - 1;

            // Process each face that should have at least one redstone, facing, below or
            // on top of the faced block.
            for face in FACES {
                if node.links.contains(face) {

                    let face_pos = node_pos + face.delta();
                    if let Some(face_node) = nodes.get_mut(&face_pos) {
                        face_node.power = face_node.power.max(propagated_power);
                        sources.push(face_pos);
                    }

                    // Only propagate upward if the block above is not opaque.
                    if !node.opaque_above {
                        let face_above_pos = face_pos + IVec3::Y;
                        if let Some(face_above_node) = nodes.get_mut(&face_above_pos) {
                            face_above_node.power = face_above_node.power.max(propagated_power);
                            sources.push(face_above_pos);
                        }
                    }

                    // Only propagate below if the block below is opaque.
                    if node.opaque_below {
                        let face_below_pos = face_pos - IVec3::Y;
                        if let Some(face_below_node) = nodes.get_mut(&face_below_pos) {
                            face_below_node.power = face_below_node.power.max(propagated_power);
                            sources.push(face_below_pos);
                        }
                    }

                }
            }

        }

    }

    // When there are no remaining power to apply, just set all remaining nodes to off.
    for node_pos in nodes.into_keys() {
        // Only notify if block has changed.
        if world.set_block(node_pos, block::REDSTONE, 0) != Some((block::REDSTONE, 0)) {
            changed_nodes.push(node_pos);
        }
    }
    
    // The following closure allows notifying a block only once, when first needed. This
    // allows us to just notify blocks around an updated redstone. The closer to a source
    // a redstone is, the earlier blocks around are notified.
    let mut notified = HashSet::new();
    let mut inner_notify_at = move |world: &mut World, pos: IVec3| {
        if notified.insert(pos) {
            notify_at(world, pos, false);
        }
    };

    // Once all blocks have been updated, notify everything.
    for node_pos in changed_nodes {
        inner_notify_at(world, node_pos + IVec3::Y);
        inner_notify_at(world, node_pos - IVec3::Y);
        inner_notify_at(world, node_pos + IVec3::Y * 2);
        inner_notify_at(world, node_pos - IVec3::Y * 2);
        for face in FACES {
            let face_pos = node_pos + face.delta();
            inner_notify_at(world, face_pos);
            inner_notify_at(world, face_pos + face.delta());
            inner_notify_at(world, face_pos + IVec3::Y);
            inner_notify_at(world, face_pos - IVec3::Y);
            inner_notify_at(world, face_pos + face.rotate_right().delta());
        }
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

/// Notification of a redstone repeater block.
fn notify_redstone_torch(world: &mut World, pos: IVec3, id: u8) {
    world.schedule_tick(pos, id, 2);
}

/// Notification of a moving fluid block.
fn notify_fluid_moving(world: &mut World, pos: IVec3, id: u8) {
    // TOOD: Make obsidian or cobblestone.
}

/// Notification of a still fluid block.
fn notify_fluid_still(world: &mut World, pos: IVec3, id: u8, metadata: u8) {

    notify_fluid_moving(world, pos, id);

    // Subtract 1 from id to go from still to moving.
    world.set_block_notify(pos, id - 1, metadata);

}
