//! Piston behaviors.

use crate::util::Face;


/// Get the facing of the piston base or extension.
pub fn get_face(metadata: u8) -> Option<Face> {
    Some(match metadata & 7 {
        0 => Face::NegY,
        1 => Face::PosY,
        2 => Face::NegZ,
        3 => Face::PosZ,
        4 => Face::NegX,
        5 => Face::PosX,
        _ => return None
    })
}
