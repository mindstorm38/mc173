//! Block ticking functions.

use glam::{IVec3, DVec3};

use crate::entity::{Entity, ItemEntity};
use crate::block_entity::BlockEntity;
use crate::block::sapling::TreeKind;
use crate::util::{Face, FaceSet};
use crate::gen::TreeGenerator;
use crate::{block, item};

use super::{World, Dimension, Event, BlockEntityEvent, BlockEntityStorage};


impl World {

    /// Tick a block in the world. The random 
    pub(super) fn tick_block_unchecked(&mut self, pos: IVec3, id: u8, metadata: u8, random: bool) {
        match id {
            // PARITY: Notchian client has random tick on button?
            block::BUTTON if !random => self.tick_button(pos, metadata),
            block::REPEATER if !random => self.tick_repeater(pos, metadata, false),
            block::REPEATER_LIT if !random => self.tick_repeater(pos, metadata, true),
            // PARITY: Notchian client have random tick on redstone torch?
            block::REDSTONE_TORCH if !random => self.tick_redstone_torch(pos, metadata, false),
            block::REDSTONE_TORCH_LIT if !random => self.tick_redstone_torch(pos, metadata, true),
            block::DISPENSER if !random => self.tick_dispenser(pos, metadata),
            block::WATER_MOVING => self.tick_fluid_moving(pos, block::WATER_MOVING, metadata),
            block::LAVA_MOVING => self.tick_fluid_moving(pos, block::LAVA_MOVING, metadata),
            // NOTE: Sugar canes and cactus have the same logic, we just give the block.
            block::SUGAR_CANES |
            block::CACTUS => self.tick_cactus_or_sugar_canes(pos, id, metadata),
            block::CAKE => {}, // Seems unused in MC
            block::WHEAT => self.tick_wheat(pos, metadata),
            block::DETECTOR_RAIL => {},
            block::FARMLAND => {},
            block::FIRE => {},
            // PARITY: Notchian client check if flowers can stay, we intentionally don't
            // respect that to allow glitched plants to stay.
            block::DANDELION |
            block::POPPY |
            block::DEAD_BUSH |
            block::TALL_GRASS => {},
            // Mushrooms ticking
            block::RED_MUSHROOM |
            block::BROWN_MUSHROOM => self.tick_mushroom(pos, id),
            block::SAPLING => self.tick_sapling(pos, metadata),
            block::GRASS => {}, // Spread
            block::ICE => {}, // Melt
            block::LEAVES => {}, // Decay
            block::WOOD_PRESSURE_PLATE |
            block::STONE_PRESSURE_PLATE => {}, // Weird, why random tick for redstone?
            block::PUMPKIN |
            block::PUMPKIN_LIT => {}, // Seems unused
            block::REDSTONE_ORE_LIT => self.tick_redstone_ore_lit(pos),
            block::SNOW => {}, // Melt
            block::SNOW_BLOCK => {}, // Melt (didn't know wtf?)
            block::LAVA_STILL => {}, // Specific to lava still
            block::TORCH => {}, // Seems not relevant..
            _ => {}
        }
    }

    /// Tick a button block, this is used to deactivate the button after 20 ticks.
    fn tick_button(&mut self, pos: IVec3, mut metadata: u8) {
        if block::button::is_active(metadata) {
            block::button::set_active(&mut metadata, false);
            self.set_block_notify(pos, block::BUTTON, metadata);
        }
    }

    fn tick_repeater(&mut self, pos: IVec3, metadata: u8, lit: bool) {

        let face = block::repeater::get_face(metadata);
        let delay = block::repeater::get_delay_ticks(metadata);
        let back_powered = self.has_passive_power_from(pos - face.delta(), face);

        if lit && !back_powered {
            self.set_block_notify(pos, block::REPEATER, metadata);
        } else if !lit {
            if !back_powered {
                self.schedule_tick(pos, block::REPEATER_LIT, delay);
            }
            self.set_block_notify(pos, block::REPEATER_LIT, metadata);
        }

    }

    fn tick_redstone_torch(&mut self, pos: IVec3, metadata: u8, lit: bool) {

        // TODO: Check torch burnout...

        let Some(torch_face) = block::torch::get_face(metadata) else { return };
        let powered = self.has_passive_power_from(pos + torch_face.delta(), torch_face.opposite());

        if lit {
            if powered {
                self.set_block_notify(pos, block::REDSTONE_TORCH, metadata);
            }
        } else {
            if !powered {
                self.set_block_notify(pos, block::REDSTONE_TORCH_LIT, metadata);
            }
        }

    }

    fn tick_dispenser(&mut self, pos: IVec3, metadata: u8) {

        let Some(face) = block::dispenser::get_face(metadata) else { return };

        // TODO: Also check for power above? (likely quasi connectivity?)

        if !self.has_passive_power(pos) {
            return;
        }

        let Some(BlockEntity::Dispenser(dispenser)) = self.get_block_entity_mut(pos) else { return };

        if let Some(index) = dispenser.pick_random_index() {

            let mut stack = dispenser.inv[index];
            let dispense_stack = stack.with_size(1);
            stack.size -= 1;
            stack = stack.to_non_empty().unwrap_or_default();
            dispenser.inv[index] = stack;

            self.push_event(Event::BlockEntity { 
                pos, 
                inner: BlockEntityEvent::Storage { 
                    storage: BlockEntityStorage::Standard(index as u8),
                    stack,
                },
            });

            let origin_pos = pos.as_dvec3() + face.delta().as_dvec3() * 0.6 + 0.5;

            if dispense_stack.id == item::ARROW {
                println!("[WARN] TODO: Shot arrow");
            } else if dispense_stack.id == item::EGG {
                println!("[WARN] TODO: Shot egg");
            } else if dispense_stack.id == item::SNOWBALL {
                println!("[WARN] TODO: Shot snowball");
            } else {

                let mut item_base = ItemEntity::default();
                item_base.kind.stack = dispense_stack;
                item_base.pos = origin_pos - DVec3::Y * 0.3;

                let rand_vel = self.rand.next_double() * 0.1 + 0.2;
                item_base.vel = face.delta().as_dvec3() * rand_vel;
                item_base.vel += self.rand.next_gaussian_dvec3() * 0.0075 * 6.0;

                self.spawn_entity(Entity::Item(item_base));

                // TODO: Play effect 1000 (click with pitch 1.0)

            }

        } else {
            // TODO: Play effect 1001 (click with pitch 1.2) in world.
        }

    }

    /// Tick a cactus.
    fn tick_cactus_or_sugar_canes(&mut self, pos: IVec3, id: u8, metadata: u8) {

        // If the block above is air, count how many cactus block are below.
        if self.is_block_air(pos + IVec3::Y) {
            
            for dy in 1.. {
                if !self.is_block(pos - IVec3::new(0, dy, 0), id) {
                    break;
                } else if dy == 2 {
                    // Two cactus blocks below, should not grow more.
                    return;
                }
            }

            if metadata == 15 {
                self.set_block_notify(pos + IVec3::Y, id, 0);
                self.set_block_notify(pos, id, 0);
            } else {
                self.set_block_notify(pos, id, metadata + 1);
            }

        }

    }

    /// Tick a wheat crop, grow it if possible.
    fn tick_wheat(&mut self, pos: IVec3, metadata: u8) {

        // Do not tick if light level is too low or already fully grown.
        let Some(light) = self.get_light(pos, true) else { return };
        if light.max < 9 || metadata >= 7 {
            return;
        }

        // Growth rate.
        let mut rate = 1.0;
        
        // Check each block below and add to the rate depending on its type.
        for x in pos.x - 1..=pos.x + 1 {
            for z in pos.z - 1..=pos.z + 1 {

                let below_pos = IVec3::new(x, pos.y - 1, z);
                if let Some((below_id, below_metadata)) = self.get_block(below_pos) {
                    
                    let mut below_rate = match (below_id, below_metadata) {
                        (block::FARMLAND, 0) => 1.0,
                        (block::FARMLAND, _) => 3.0,
                        _ => continue,
                    };

                    if x != pos.x || z != pos.z {
                        below_rate /= 4.0;
                    }
                    
                    rate += below_rate;

                }

            }
        }
        
        // Calculate the growth rate, it depends on surrounding wheat crops.
        let mut same_faces = FaceSet::new();
        let mut same_corner = false;

        for face in Face::HORIZONTAL {
            let face_pos = pos + face.delta();
            if matches!(self.get_block(face_pos), Some((block::WHEAT, _))) {
                same_faces.insert(face);
            }
            let corner_pos = face_pos + face.rotate_right().delta();
            if matches!(self.get_block(corner_pos), Some((block::WHEAT, _))) {
                // Same corner is enough to divide the growth rate, so we break here.
                same_corner = true;
                break;
            }
        }
        
        if same_corner || (same_faces.contains_x() && same_faces.contains_z()) {
            rate /= 2.0;
        }

        // Randomly grow depending on the calculated rate.
        if self.rand.next_int_bounded((100.0 / rate) as i32) == 0 {
            self.set_block_notify(pos, block::WHEAT, metadata + 1);
        }

    }

    /// Tick a mushroom to try spreading it.
    fn tick_mushroom(&mut self, pos: IVec3, id: u8) {
        if self.rand.next_int_bounded(100) == 0 {

            let spread_pos = pos + IVec3 {
                x: self.rand.next_int_bounded(3) - 1,
                y: self.rand.next_int_bounded(2) - self.rand.next_int_bounded(2),
                z: self.rand.next_int_bounded(3) - 1,
            };

            if let Some(light) = self.get_light(spread_pos, false) {
                if light.max < 13 {
                    if self.is_block_air(spread_pos) {
                        if self.is_block_opaque_cube(spread_pos - IVec3::Y) {
                            self.set_block_notify(spread_pos, id, 0);
                        }
                    }
                }
            }

        }
    }

    /// Tick a sapling to grow it.
    fn tick_sapling(&mut self, pos: IVec3, mut metadata: u8) {
        if let Some(light) = self.get_light(pos + IVec3::Y, true) {
            if light.max >= 9 && self.rand.next_int_bounded(30) == 0 {
                if block::sapling::is_growing(metadata) {
                   
                    let mut gen = match block::sapling::get_kind(metadata) {
                        TreeKind::Oak if self.rand.next_int_bounded(10) == 0 => TreeGenerator::new_big(),
                        TreeKind::Oak => TreeGenerator::new_oak(),
                        TreeKind::Birch => TreeGenerator::new_birch(),
                        TreeKind::Spruce => TreeGenerator::new_spruce2(),
                    };

                    gen.generate_from_sapling(self, pos);

                } else {
                    block::sapling::set_growing(&mut metadata, true);
                    self.set_block_notify(pos, block::SAPLING, metadata);
                }
            }
        }
    }

    fn tick_redstone_ore_lit(&mut self, pos: IVec3) {
        self.set_block_notify(pos, block::REDSTONE_ORE, 0);
    }

    /// Tick a moving fluid block.
    fn tick_fluid_moving(&mut self, pos: IVec3, flowing_id: u8, mut metadata: u8) {

        // +1 to get still fluid id.
        let still_id = flowing_id + 1;

        // Default distance to decrement on each block unit.
        let dist_drop = match flowing_id {
            block::LAVA_MOVING if self.get_dimension() != Dimension::Nether => 2,
            _ => 1,
        };

        // The id below is used many time after, so we query it here.
        let below_pos = pos - IVec3::Y;
        let (below_id, below_metadata) = self.get_block(below_pos).unwrap_or((block::AIR, 0));

        // Update this fluid state.
        if !block::fluid::is_source(metadata) {

            // Default to 8, so if no fluid block is found around, fluid will disappear.
            let mut shortest_dist = 8;
            let mut sources_around = 0u8;

            for face in [Face::NegX, Face::PosX, Face::NegZ, Face::PosZ] {
                if let Some((face_id, face_metadata)) = self.get_block(pos + face.delta()) {
                    // Only if this block is of the same type.
                    // +1 to get the "still" id.
                    if face_id == flowing_id || face_id == still_id {
                        let face_dist = block::fluid::get_actual_distance(face_metadata);
                        shortest_dist = shortest_dist.min(face_dist);
                        if block::fluid::is_source(face_metadata) {
                            sources_around += 1;
                        }
                    }
                }
            }

            let mut new_metadata = shortest_dist + dist_drop;
            if new_metadata > 7 {
                // Just mark that the metadata is invalid, fluid should disappear.
                new_metadata = 0xFF;
            }

            // If the top block on top is the same fluid, this become a falling state fluid.
            if let Some((above_id, above_metadata)) = self.get_block(pos + IVec3::Y) {
                if above_id == flowing_id || above_id == still_id {
                    // Copy the above metadata but force falling state.
                    new_metadata = above_metadata;
                    block::fluid::set_falling(&mut new_metadata, true);
                }
            }

            // Infinite water sources!
            if sources_around >= 2 && flowing_id == block::WATER_MOVING {
                if block::from_id(below_id).material.is_solid() {
                    block::fluid::set_source(&mut new_metadata);
                } else if below_id == flowing_id || below_id == still_id {
                    if block::fluid::is_source(below_metadata) {
                        block::fluid::set_source(&mut new_metadata);
                    }
                }
            }

            // TODO: Weird lava stuff.

            if new_metadata != metadata {
                metadata = new_metadata;
                if new_metadata == 0xFF {
                    self.set_block_notify(pos, block::AIR, 0);
                } else {
                    self.set_block_notify(pos, flowing_id, new_metadata);
                }
            } else {
                // Metadata is the same, set still.
                self.set_block(pos, still_id, metadata);
            }

        } else {
            // Moving source is systematically set to still source.
            self.set_block(pos, still_id, metadata);
        }

        // The block has been removed, don't propagate it.
        if metadata == 0xFF {
            return;
        }

        let blocked_below = block::fluid::is_fluid_blocked(below_id);
        if !block::fluid::is_fluid_block(below_id) && !blocked_below {
            // The block below is not a fluid block and do not block fluids, the fluid below
            // is set to a falling version of the current block.
            block::fluid::set_falling(&mut metadata, true);
            self.set_block_notify(below_pos, flowing_id, metadata);
        } else if block::fluid::is_source(metadata) || blocked_below {

            // The block is a source or is blocked below, we spread it horizontally.
            // let open_faces = FaceSet::new();
            // for face in [Face::NegX, Face::PosX, Face::NegZ, Face::PosZ] {
            //     if let Some((face_id, face_metadata)) = world.block(pos + face.delta()) {
            //         if !block::fluid::is_fluid_blocked(face_id) {
            //             if block::fluid::is_source(face_metadata) || (face_id != moving_id && face_id != still_id) {

            //             }
            //         }
            //     }
            // }

            // TODO: Algorithm to determine the flow direction.

            let new_dist = block::fluid::get_actual_distance(metadata) + dist_drop;
            if new_dist > 7 {
                return;
            }

            for face in [Face::NegX, Face::PosX, Face::NegZ, Face::PosZ] {
                let face_pos = pos + face.delta();
                if let Some((face_id, _)) = self.get_block(face_pos) {
                    if !block::fluid::is_fluid_block(face_id) && !block::fluid::is_fluid_blocked(face_id) {
                        // TODO: Break only for water.
                        self.break_block(face_pos);
                        self.set_block_notify(face_pos, flowing_id, new_dist);
                    }
                }
            }

        }

    }

}
