//! Global redstone powering behaviors.

use glam::IVec3;

use crate::world::World;
use crate::util::{Face, FaceSet};
use crate::block;


/// Return true if the block at given position can link to a redstone wire from its given
/// face.
pub fn is_linkable_from(world: &mut World, pos: IVec3, face: Face) -> bool {
    
    let Some((id, metadata)) = world.block(pos) else { return false };

    match id {
        block::LEVER |
        block::BUTTON |
        block::DETECTOR_RAIL |
        block::WOOD_PRESSURE_PLATE |
        block::STONE_PRESSURE_PLATE |
        block::REDSTONE_TORCH |
        block::REDSTONE_TORCH_LIT |
        block::REDSTONE => true,
        block::REPEATER |
        block::REPEATER_LIT => {
            let repeater_face = block::repeater::get_face(metadata);
            face == repeater_face
        }
        _ => false
    }

}


/// Get the active power produced by a block's face.
pub fn get_active_power_from(world: &mut World, pos: IVec3, face: Face) -> u8 {
    let power = get_power_from(world, pos, face, true);
    if power.indirect || !power.passive {
        power.level
    } else {
        0
    }
}

/// Get the passive power of a block's face.
pub fn get_passive_power_from(world: &mut World, pos: IVec3, face: Face) -> u8 {
    get_power_from(world, pos, face, true).level
}

/// Get the power produced by a block on a given face.
fn get_power_from(world: &mut World, pos: IVec3, face: Face, test_block: bool) -> Power {

    let Some((id, metadata)) = world.block(pos) else { return Power::OFF };

    match id {
        block::LEVER => get_lever_power_from(face, metadata),
        block::BUTTON => get_button_power_from(face, metadata),
        block::REPEATER_LIT => get_repeater_power_from(face, metadata),
        block::REDSTONE_TORCH_LIT => get_redstone_torch_power_from(face, metadata),
        block::REDSTONE => get_redstone_power_from(world, pos, face, metadata),
        // Opaque block relaying indirect power 
        _ if test_block && block::material::is_opaque_cube(id) => 
            get_block_power_from(world, pos, face),
        // Non-redstone blocks
        _ => Power::OFF
    }

}

/// Get the power of a block that would be indirectly powered.
fn get_block_power_from(world: &mut World, pos: IVec3, face: Face) -> Power {

    // By default the block is passive, but if a face has a non-passive power then is 
    // will no longer be passive.
    let mut ret = Power { level: 0, indirect: false, passive: true };

    // Find the maximum 
    for test_face in [Face::NegY, Face::PosY, Face::NegZ, Face::PosZ, Face::NegX, Face::PosX] {
        if test_face != face {

            // Test the power coming from that face, but disable 'test_block' to avoid
            // infinite recursion between those two functions, this assumption is valid
            // because a block cannot retransmit other block's power.
            let power = get_power_from(world, pos + test_face.delta(), test_face.opposite(), false);
            // Only relay the power if the face provides indirect power.
            if power.indirect {

                if !power.passive && ret.passive {
                    ret.level = power.level;
                    ret.passive = false;
                } else if power.passive == ret.passive && power.level > ret.level {
                    ret.level = power.level;
                }

                // If return value is not passive and already maximum level, return.
                if !ret.passive && ret.level >= 15 {
                    break;
                }

            }

        }
    }

    ret

}

fn get_lever_power_from(face: Face, metadata: u8) -> Power {
    if block::lever::is_active(metadata) {
        if block::lever::get_face(metadata).map(|(f, _)| f) == Some(face) {
            Power::ON_INDIRECT
        } else {
            Power::ON_DIRECT
        }
    } else {
        Power::OFF
    }
}

fn get_button_power_from(face: Face, metadata: u8) -> Power {
    if block::button::is_active(metadata) {
        if block::button::get_face(metadata) == Some(face) {
            Power::ON_INDIRECT
        } else {
            Power::ON_DIRECT
        }
    } else {
        Power::OFF
    }
}

fn get_repeater_power_from(face: Face, metadata: u8) -> Power {
    if block::repeater::get_face(metadata) == face {
        Power::ON_INDIRECT
    } else {
        Power::OFF
    }
}

fn get_redstone_torch_power_from(face: Face, metadata: u8) -> Power {
    if block::torch::get_face(metadata) == Some(face) {
        Power::OFF
    } else if face == Face::PosY {
        Power::ON_INDIRECT
    } else {
        Power::ON_DIRECT
    }
}

fn get_redstone_power_from(world: &mut World, pos: IVec3, face: Face, metadata: u8) -> Power {
    if face == Face::PosY || metadata == 0 {
        Power::OFF
    } else if face == Face::NegY {
        Power { level: metadata, indirect: true, passive: true }
    } else {

        let mut links = FaceSet::new();

        let opaque_above = world.block(pos + IVec3::Y)
            .map(|(above_id, _)| block::material::is_opaque_cube(above_id))
            .unwrap_or(true);

        for face in [Face::NegX, Face::PosX, Face::NegZ, Face::PosZ] {
            let face_pos = pos + face.delta();
            if is_linkable_from(world, face_pos, face.opposite()) {
                links.insert(face);
            } else {
                if let Some((id, _)) = world.block(face_pos) {
                    if !block::material::is_opaque_cube(id) {
                        if is_linkable_from(world, face_pos - IVec3::Y, Face::PosY) {
                            links.insert(face);
                        }
                    } else if !opaque_above {
                        if is_linkable_from(world, face_pos + IVec3::Y, Face::NegY) {
                            links.insert(face);
                        }
                    }
                }
            }
        }

        // Check if the redstone wire is directly pointing to its horizontal faces,
        // if so the current is indirect and can be transmitted through the face block,
        // if not it is just a passive signal that can be detected by repeaters.
        let indirect = if links.is_empty() {
            // The redstone wire has no links, so it has a cross shape and provide power
            // to all sides.
            true
        } else {
            match face {
                Face::NegZ => links.contains(Face::PosZ) && !links.contains_x(),
                Face::PosZ => links.contains(Face::NegZ) && !links.contains_x(),
                Face::NegX => links.contains(Face::PosX) && !links.contains_z(),
                Face::PosX => links.contains(Face::NegX) && !links.contains_z(),
                _ => unreachable!()
            }
        };

        Power { level: metadata, indirect, passive: true }

    }
}


#[derive(Debug)]
struct Power {
    /// The redstone power level (0..16).
    level: u8,
    /// If this power can be relayed indirectly by opaque blocks.
    indirect: bool,
    /// If this power is passive.
    passive: bool,
}

impl Power {

    const OFF: Self = Self { level: 0, indirect: false, passive: false };
    const ON_INDIRECT: Self = Self { level: 15, indirect: true, passive: false };
    const ON_DIRECT: Self = Self { level: 15, indirect: false, passive: false };

}
