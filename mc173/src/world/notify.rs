//! Block notification and tick methods for world.

use std::collections::{HashMap, HashSet};
use std::collections::hash_map::Entry;

use glam::IVec3;

use crate::block::material::PistonPolicy;
use crate::block_entity::piston::PistonBlockEntity;
use crate::block_entity::BlockEntity;
use crate::geom::{Face, FaceSet};
use crate::block;

use super::{World, Event, BlockEvent};


/// Methods related to block self and neighbor notifications.
impl World {

    /// Notify all blocks around the position, the notification origin block id is given.
    pub fn notify_blocks_around(&mut self, pos: IVec3, origin_id: u8) {
        for face in Face::ALL {
            self.notify_block(pos + face.delta(), origin_id);
        }
    }

    /// Notify a block a the position, the notification origin block id is given.
    pub fn notify_block(&mut self, pos: IVec3, origin_id: u8) {
        if let Some((id, metadata)) = self.get_block(pos) {
            self.notify_block_unchecked(pos, id, metadata, origin_id);
        }
    }

    /// Notify a block a the position, the notification origin block id is given.
    pub(super) fn notify_block_unchecked(&mut self, pos: IVec3, id: u8, metadata: u8, origin_id: u8) {
        match id {
            block::REDSTONE if origin_id != block::REDSTONE => self.notify_redstone(pos),
            block::REPEATER |
            block::REPEATER_LIT => self.notify_repeater(pos, id, metadata),
            block::REDSTONE_TORCH |
            block::REDSTONE_TORCH_LIT => self.notify_redstone_torch(pos, id),
            block::DISPENSER => self.notify_dispenser(pos, origin_id),
            block::WATER_MOVING |
            block::LAVA_MOVING => self.notify_fluid(pos, id, metadata),
            block::WATER_STILL |
            block::LAVA_STILL => self.notify_fluid_still(pos, id, metadata),
            block::TRAPDOOR => self.notify_trapdoor(pos, metadata, origin_id),
            block::WOOD_DOOR |
            block::IRON_DOOR => self.notify_door(pos, id, metadata, origin_id),
            block::DANDELION |
            block::POPPY |
            block::SAPLING |
            block::TALL_GRASS => self.notify_flower(pos, &[block::GRASS, block::DIRT, block::FARMLAND]),
            block::DEAD_BUSH => self.notify_flower(pos, &[block::SAND]),
            block::WHEAT => self.notify_flower(pos, &[block::FARMLAND]),
            block::RED_MUSHROOM |
            block::BROWN_MUSHROOM => self.notify_mushroom(pos),
            block::CACTUS => self.notify_cactus(pos),
            block::SAND |
            block::GRAVEL => self.schedule_block_tick(pos, id, 3),
            block::FIRE => { self.notify_fire(pos); },
            block::PISTON |
            block::STICKY_PISTON => self.notify_piston(pos, id, metadata),
            block::PISTON_EXT => self.notify_piston_ext(pos, metadata, origin_id),
            block::NOTE_BLOCK => self.notify_note_block(pos, origin_id),
            _ => {}
        }
    }

    pub(super) fn notify_change_unchecked(&mut self, pos: IVec3, 
        from_id: u8, from_metadata: u8,
        to_id: u8, to_metadata: u8
    ) {

        match from_id {
            block::BUTTON => {
                if let Some(face) = block::button::get_face(from_metadata) {
                    self.notify_blocks_around(pos + face.delta(), block::BUTTON);
                }
            }
            block::LEVER => {
                if let Some((face, _)) = block::lever::get_face(from_metadata) {
                    self.notify_blocks_around(pos + face.delta(), block::LEVER);
                }
            }
            // Remove the chest/dispenser block entity.
            block::CHEST if to_id != block::CHEST => { 
                self.remove_block_entity(pos); 
            }
            block::DISPENSER if to_id != block::DISPENSER => { 
                self.remove_block_entity(pos);
            }
            // Remove the furnace block entity.
            block::FURNACE |
            block::FURNACE_LIT if to_id != block::FURNACE_LIT && to_id != block::FURNACE => {
                self.remove_block_entity(pos);
            }
            block::SPAWNER if to_id != block::SPAWNER => {
                self.remove_block_entity(pos);
            }
            block::NOTE_BLOCK if to_id != block::NOTE_BLOCK => {
                self.remove_block_entity(pos);
            }
            block::JUKEBOX if to_id != block::JUKEBOX => {
                self.remove_block_entity(pos);
            }
            _ => {}
        }

        match to_id {
            block::WATER_MOVING => self.schedule_block_tick(pos, to_id, 5),
            block::LAVA_MOVING => self.schedule_block_tick(pos, to_id, 30),
            block::REDSTONE => self.notify_redstone(pos),
            block::REPEATER |
            block::REPEATER_LIT => self.notify_repeater(pos, to_id, from_metadata),
            block::REDSTONE_TORCH |
            block::REDSTONE_TORCH_LIT => self.notify_redstone_torch(pos, to_id),
            block::SAND |
            block::GRAVEL => self.schedule_block_tick(pos, to_id, 3),
            block::CACTUS => self.notify_cactus(pos),
            block::FIRE => self.notify_fire_place(pos),
            block::PISTON |
            block::STICKY_PISTON => self.notify_piston(pos, to_id, to_metadata),
            _ => {}
        }

    }

    /// Notification of a moving fluid block.
    fn notify_fluid(&mut self, pos: IVec3, id: u8, metadata: u8) {
        // If the fluid block is lava, check if we make cobblestone or lava.
        if id == block::LAVA_MOVING {
            let distance = block::fluid::get_distance(metadata);
            for face in Face::HORIZONTAL {
                if let Some((block::WATER_MOVING | block::WATER_STILL, _)) = self.get_block(pos + face.delta()) {
                    // If there is at least one water block around.
                    if distance == 0 {
                        self.set_block_notify(pos, block::OBSIDIAN, 0);
                    } else if distance <= 4 {
                        self.set_block_notify(pos, block::COBBLESTONE, 0);
                    }
                }
            }
        }
    }

    /// Notification of a still fluid block.
    fn notify_fluid_still(&mut self, pos: IVec3, id: u8, metadata: u8) {

        // Subtract 1 from id to go from still to moving.
        let moving_id = id - 1;

        self.notify_fluid(pos, moving_id, metadata);
        self.set_block_self_notify(pos, moving_id, metadata);

    }

    /// Notification of standard flower subclasses.
    fn notify_flower(&mut self, pos: IVec3, stay_blocks: &[u8]) {
        if self.get_light(pos).max() >= 8 || false /* TODO: block can see sky */ {
            let (below_id, _) = self.get_block(pos - IVec3::Y).unwrap_or((0, 0));
            if stay_blocks.iter().any(|&id| id == below_id) {
                return;
            }
        }
        self.break_block(pos);
    }

    /// Notification of a mushroom block.
    fn notify_mushroom(&mut self, pos: IVec3) {
        if self.get_light(pos).max() >= 13 || !self.is_block_opaque_cube(pos - IVec3::Y) {
            self.break_block(pos);
        }
    }

    /// Notification of a cactus block. The block is broken if 
    fn notify_cactus(&mut self, pos: IVec3) {
        for face in Face::HORIZONTAL {
            if self.is_block_solid(pos + face.delta()) {
                self.break_block(pos);
                return;
            }
        }
        if !matches!(self.get_block(pos - IVec3::Y), Some((block::CACTUS | block::SAND, _))) {
            self.break_block(pos);
        }
    }

    /// Notification of a fire block, the fire block is removed if the block below is no
    /// longer a normal cube wall blocks cannot catch fire.
    /// 
    /// This function returns true if the fire has been removed (internally used).
    fn notify_fire(&mut self, pos: IVec3) -> bool {

        for face in Face::ALL {
            if let Some((id, _)) = self.get_block(pos + face.delta()) {
                if face == Face::NegY && block::material::is_normal_cube(id) {
                    return false;
                } else if block::material::get_fire_flammability(id) != 0 {
                    return false;
                }
            }
        }

        self.set_block_notify(pos, block::AIR, 0);
        true

    }

    /// Notification of a fire block being placed.
    fn notify_fire_place(&mut self, pos: IVec3) {
        
        if self.notify_fire(pos) {
            return;
        }

        // Check where there is obsidian around.
        let obsidians = Face::HORIZONTAL.into_iter()
            .filter(|face| self.is_block(pos + face.delta(), block::OBSIDIAN))
            .collect::<FaceSet>();

        // If only one side has obsidian, check to create a nether portal.
        if obsidians.contains_x() != obsidians.contains_z() {
            
            // Portal origin to lower X/Z
            let mut pos = pos;
            if obsidians.contains(Face::PosX) {
                pos.x -= 1;
            } else if obsidians.contains(Face::PosZ) {
                pos.z -= 1;
            }

            let factor = IVec3 {
                x: obsidians.contains_x() as i32,
                y: 1,
                z: obsidians.contains_z() as i32,
            };

            let mut valid = true;
            for dxz in -1..=2 {
                for dy in -1..=3 {
                    if (dxz != -1 && dxz != 2) || (dy != -1 && dy != 3) {
                        
                        let Some((id, _)) = self.get_block(pos + factor * IVec3::new(dxz, dy, dxz)) else {
                            valid = false;
                            break;
                        };

                        if dxz == -1 || dxz == 2 || dy == -1 || dy == 3 {
                            if id != block::OBSIDIAN {
                                valid = false;
                                break;
                            }
                        } else if id != block::AIR && id != block::FIRE {
                            valid = false;
                            break;
                        }

                    }
                }
            }

            // If portal layout is valid, create it.
            if valid {
                for dxz in 0..2 {
                    for dy in 0..3 {
                        self.set_block_notify(pos + factor * IVec3::new(dxz, dy, dxz), block::PORTAL, 0);
                    }
                }
                return;
            }

        }

        // Fallback to regular fire placing, just schedule a fire tick.
        self.schedule_block_tick(pos, block::FIRE, 40)

    }

    /// Notification of a redstone repeater block.
    fn notify_repeater(&mut self, pos: IVec3, id: u8, metadata: u8) {

        let lit = id == block::REPEATER_LIT;
        let face = block::repeater::get_face(metadata);
        let delay = block::repeater::get_delay_ticks(metadata);
        let back_powered = self.has_passive_power_from(pos - face.delta(), face);

        if lit != back_powered {
            self.schedule_block_tick(pos, id, delay);
        }

    }

    /// Notification of a redstone repeater block.
    fn notify_redstone_torch(&mut self, pos: IVec3, id: u8) {
        self.schedule_block_tick(pos, id, 2);
    }

    fn notify_dispenser(&mut self, pos: IVec3, origin_id: u8) {
        if is_redstone_block(origin_id) {
            // TODO: Also check above? See associated tick function.
            if self.has_passive_power(pos) {
                self.schedule_block_tick(pos, block::DISPENSER, 4);
            }
        }
    }

    /// Notification of a trapdoor, breaking it if no longer on its wall, or updating its 
    /// state depending on redstone signal.
    fn notify_trapdoor(&mut self, pos: IVec3, mut metadata: u8, origin_id: u8) {
        let face = block::trapdoor::get_face(metadata);
        if !self.is_block_opaque_cube(pos + face.delta()) {
            self.break_block(pos);
        } else {
            let open = block::trapdoor::is_open(metadata);
            if is_redstone_block(origin_id) {
                let powered = self.has_passive_power(pos);
                if open != powered {
                    block::trapdoor::set_open(&mut metadata, powered);
                    self.set_block_notify(pos, block::TRAPDOOR, metadata);
                    self.push_event(Event::Block { 
                        pos, 
                        inner: BlockEvent::Sound { id: block::TRAPDOOR, metadata },
                    });
                }
            }
        }
    }

    fn notify_door(&mut self, pos: IVec3, id: u8, mut metadata: u8, origin_id: u8) {

        if block::door::is_upper(metadata) {
            
            // If the block below is not another door,
            if let Some((below_id, below_metadata)) = self.get_block(pos - IVec3::Y) {
                if below_id == id {
                    self.notify_door(pos - IVec3::Y, below_id, below_metadata, origin_id);
                    return;
                }
            }

            // Do not naturally break, top door do not drop anyway.
            self.set_block_notify(pos, block::AIR, 0);

        } else {

            // If the block above is not the same door block, naturally break itself.
            if let Some((above_id, _)) = self.get_block(pos + IVec3::Y) {
                if above_id != id {
                    self.break_block(pos);
                    return;
                }
            }

            // Also check that door can stay in place.
            if !self.is_block_opaque_cube(pos - IVec3::Y) {
                // NOTE: This will notify the upper part and destroy it.
                self.break_block(pos);
                return;
            }

            if is_redstone_block(origin_id) {

                // Check if the door is powered in any way.
                let mut powered = 
                    self.has_passive_power_from(pos - IVec3::Y, Face::PosY) ||
                    self.has_passive_power_from(pos + IVec3::Y * 2, Face::NegY);

                if !powered {
                    for face in Face::ALL {
                        let face_pos = pos + face.delta();
                        powered = 
                            self.has_passive_power_from(face_pos, face.opposite()) || 
                            self.has_passive_power_from(face_pos + IVec3::Y, face.opposite());
                        if powered {
                            break;
                        }
                    }
                }
                
                // Here we know that the current and above blocks are the same door type, we can
                // simply set the metadata of the two. Only update if needed.
                if block::door::is_open(metadata) != powered {

                    block::door::set_open(&mut metadata, powered);

                    // Do not use notify methods to avoid updating the upper half.
                    self.set_block_self_notify(pos, id, metadata);
                    block::door::set_upper(&mut metadata, true);
                    self.set_block_self_notify(pos + IVec3::Y, id, metadata);

                    self.notify_block(pos - IVec3::Y, id);
                    self.notify_block(pos + IVec3::Y * 2, id);
                    for face in Face::ALL {
                        self.notify_block(pos + face.delta(), id);
                        self.notify_block(pos + face.delta() + IVec3::Y, id);
                    }

                    self.push_event(Event::Block { 
                        pos, 
                        inner: BlockEvent::Sound { id, metadata },
                    });

                }
                
            }

        }

    }

    /// Notify a piston (sticky or not).
    fn notify_piston(&mut self, pos: IVec3, id: u8, metadata: u8) {

        let Some(face) = block::piston::get_face(metadata) else { return };
        let extended = block::piston::is_base_extended(metadata);
        let sticky = id == block::STICKY_PISTON;

        let powered = Face::ALL.into_iter()
            .filter(|&check_face| check_face != face)
            .any(|face| self.has_passive_power_from(pos + face.delta(), face.opposite()));

        let delta = face.delta();

        if powered != extended {

            // If powering the piston, check that this is possible.
            if powered {

                /// Push limit in block for a piston.
                const PUSH_LIMIT: usize = 12;

                // We add one (..=) in order to check that the blocks are not blocked.
                let mut check_pos = pos + delta;
                let mut push_count = 0;
                loop {

                    // PARITY: Notchian impl cannot push Y=0
                    let Some((check_id, check_metadata)) = self.get_block(check_pos) else {
                        // Abort if we are not in loaded chunk.
                        return;
                    };

                    // Abort if the block cannot be pushed.
                    match block::material::get_piston_policy(check_id, check_metadata) {
                        PistonPolicy::Break => break,
                        PistonPolicy::Stop => return,
                        PistonPolicy::PushPull => {}
                    }

                    // We the block can be pushed but we reached push limit, abort.
                    if push_count == PUSH_LIMIT {
                        return;
                    }

                    check_pos += delta;
                    push_count += 1;

                }

                // Break the last position (do not use self.break_block to avoid recurse).
                if let Some((prev_id, prev_metadata)) = self.set_block(check_pos, block::AIR, 0) {
                    self.spawn_block_loot(pos, prev_id, prev_metadata, 1.0);
                }

                // Now we initialize the block entities.
                let mut move_pos = pos + delta;
                // Follow the previous block to initialize block entities.
                let mut next_id = block::PISTON_EXT;
                let mut next_metadata = 0;
                block::piston::set_face(&mut next_metadata, face);
                block::piston::set_ext_sticky(&mut next_metadata, sticky);

                for _ in 0..=push_count {

                    let (prev_id, prev_metadata) = 
                    self.set_block(move_pos, block::PISTON_MOVING, next_metadata).unwrap();
                    self.set_block_entity(move_pos, BlockEntity::Piston(PistonBlockEntity {
                        block: next_id,
                        metadata: next_metadata,
                        face,
                        progress: 0.0,
                        extending: true,
                    }));

                    move_pos += delta;
                    next_id = prev_id;
                    next_metadata = prev_metadata;

                }

            } else {

                // Check if a piston block entity is still present on the head, we need
                // to remove it instantly and replace with its block.
                let head_pos = pos + delta;
                if let Some(BlockEntity::Piston(piston)) = self.get_block_entity_mut(head_pos) {
                    let (moving_id, moving_metadata) = (piston.block, piston.metadata);
                    self.remove_block_entity(head_pos);
                    if self.is_block(pos, block::PISTON_MOVING) {
                        self.set_block_notify(pos, moving_id, moving_metadata);
                    }
                }

                // Now we replace the piston base by a moving piston block entity.
                self.set_block(pos, block::PISTON_MOVING, metadata);
                self.set_block_entity(pos, BlockEntity::Piston(PistonBlockEntity {
                    block: id,
                    metadata,
                    face,
                    progress: 0.0,
                    extending: false,
                }));

                if sticky {

                    let sticky_pos = head_pos + delta;
                    let Some((mut sticky_id, mut sticky_metadata)) = self.get_block(sticky_pos) else {
                        // We abort if the sticky block is in unloaded chunk.
                        return;
                    };

                    // We can't retract a moving piston, we instantly place its block.
                    // This is the mechanic that allows dropping block with sticky piston.
                    let mut sticky_drop = false;
                    if sticky_id == block::PISTON_MOVING {
                        if let Some(BlockEntity::Piston(piston)) = self.get_block_entity_mut(sticky_pos) {
                            if piston.extending && piston.face == face {
                                sticky_id = piston.block;
                                sticky_metadata = sticky_metadata;
                                sticky_drop = true;
                                self.remove_block_entity(head_pos);
                                if self.is_block(pos, block::PISTON_MOVING) {
                                    self.set_block_notify(pos, sticky_id, sticky_metadata);
                                }
                            }
                        }
                    }

                    if sticky_drop || block::material::get_piston_policy(sticky_id, sticky_metadata) != PistonPolicy::PushPull {
                        self.set_block(head_pos, block::AIR, 0);
                    } else {
                        self.set_block(sticky_pos, block::AIR, 0);
                        self.set_block(head_pos, block::PISTON_MOVING, sticky_metadata);
                        self.set_block_entity(head_pos, BlockEntity::Piston(PistonBlockEntity {
                            block: sticky_id,
                            metadata: sticky_metadata,
                            face,
                            progress: 0.0,
                            extending: false,
                        }));
                    }

                } else {
                    self.set_block(head_pos, block::AIR, 0);
                }

            }

            // Set the block metadata (no notification).
            let mut metadata = metadata;
            block::piston::set_base_extended(&mut metadata, powered);
            self.set_block(pos, id, metadata);

            self.push_event(Event::Block { 
                pos, 
                inner: BlockEvent::Piston { 
                    extending: powered,
                    face,
                }
            });
            
        } else if extended {

            // If the piston has just been notified and is extended, we break it if its
            // extension has been removed.
            let head_pos = pos + delta;
            if !self.is_block(head_pos, block::PISTON_EXT) {
                self.break_block(pos);
            }

        }

    }

    /// Notify a piston extension, removing it if no piston exists.
    fn notify_piston_ext(&mut self, pos: IVec3, metadata: u8, origin_id: u8) {
        
        let Some(face) = block::piston::get_face(metadata) else { return };
        
        let base_pos = pos - face.delta();
        if let Some((base_id, base_metadata)) = self.get_block(base_pos) {
            if let block::PISTON | block::STICKY_PISTON = base_id {
                // Just forward the notification to the piston.
                self.notify_block_unchecked(base_pos, base_id, base_metadata, origin_id);
                return;
            }
        }

        self.set_block_notify(pos, block::AIR, 0);

    }

    /// Notify a note block, playing a sound if powered by redstone.
    fn notify_note_block(&mut self, pos: IVec3, origin_id: u8) {

        if !is_redstone_block(origin_id) {
            return;
        }

        let powered = self.has_passive_power(pos);
        let Some(BlockEntity::NoteBlock(note_block)) = self.get_block_entity_mut(pos) else {
            // Abort if no block entity.
            return;
        };

        if note_block.powered != powered {
            note_block.powered = powered;
            if powered {
                // Forward to block interaction.
                self.interact_block_unchecked(pos, block::NOTE_BLOCK, 0, true);
            }
        }

    }

    /// Notify a redstone dust block. This function is a bit special because this 
    /// notification in itself will trigger other notifications for all updated blocks.
    /// The redstone update in the 
    fn notify_redstone(&mut self, pos: IVec3) {

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
            node.opaque_above = self.get_block(pos + IVec3::Y)
                .map(|(above_id, _)| block::material::is_opaque_cube(above_id))
                .unwrap_or(true);
            node.opaque_below = self.get_block(pos - IVec3::Y)
                .map(|(below_id, _)| block::material::is_opaque_cube(below_id))
                .unwrap_or(true);

            for face in FACES {

                // Do not process the face that discovered this node: this avoid too many
                // recursion, and this is valid since 
                if link_face == face {
                    continue;
                }

                let face_pos = pending_pos + face.delta();
                if let Some((id, _)) = self.get_block(face_pos) {

                    if id == block::REDSTONE {
                        node.links.insert(face);
                        pending.push((face_pos, face.opposite()));
                        continue;
                    }

                    // If the faced block is not a redstone, get the direct power from it and
                    // update our node initial power depending on it.
                    let face_power = self.get_active_power_from(face_pos, face.opposite());
                    node.power = node.power.max(face_power);

                    if block::material::get_material(id).is_opaque() {
                        // If that faced block is opaque, we check if a redstone dust is 
                        // present on top of it, we connect the network to it if not opaque 
                        // above.
                        if !node.opaque_above {
                            let face_above_pos = face_pos + IVec3::Y;
                            if let Some((block::REDSTONE, _)) = self.get_block(face_above_pos) {
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
                        if let Some((block::REDSTONE, _)) = self.get_block(face_below_pos) {
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
                let face_power = self.get_active_power_from(face_pos, face.opposite());
                node.power = node.power.max(face_power);
            }

            if node.power > 0 {
                sources.push(pending_pos);
            }

        }

        // No longer used, just as a programmer hint.
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
                if self.set_block(node_pos, block::REDSTONE, node.power) != Some((block::REDSTONE, node.power)) {
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
            if self.set_block(node_pos, block::REDSTONE, 0) != Some((block::REDSTONE, 0)) {
                changed_nodes.push(node_pos);
            }
        }
        
        // The following closure allows notifying a block only once, when first needed. This
        // allows us to just notify blocks around an updated redstone. The closer to a source
        // a redstone is, the earlier blocks around are notified.
        let mut notified = HashSet::new();
        let mut inner_notify_at = move |pos: IVec3| {
            if notified.insert(pos) {
                self.notify_block(pos, block::REDSTONE);
            }
        };

        // Once all blocks have been updated, notify everything.
        for node_pos in changed_nodes {
            inner_notify_at(node_pos + IVec3::Y);
            inner_notify_at(node_pos - IVec3::Y);
            inner_notify_at(node_pos + IVec3::Y * 2);
            inner_notify_at(node_pos - IVec3::Y * 2);
            for face in FACES {
                let face_pos = node_pos + face.delta();
                inner_notify_at(face_pos);
                inner_notify_at(face_pos + face.delta());
                inner_notify_at(face_pos + IVec3::Y);
                inner_notify_at(face_pos - IVec3::Y);
                inner_notify_at(face_pos + face.rotate_right().delta());
            }
        }
        
    }

}


fn is_redstone_block(id: u8) -> bool {
    match id {
        block::BUTTON |
        block::DETECTOR_RAIL |
        block::LEVER |
        block::WOOD_PRESSURE_PLATE |
        block::STONE_PRESSURE_PLATE |
        block::REPEATER |
        block::REPEATER_LIT |
        block::REDSTONE_TORCH |
        block::REDSTONE_TORCH_LIT |
        block::REDSTONE => true,
        _ => false,
    }
}
