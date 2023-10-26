//! Piston behaviors.

use crate::util::Face;


/// Get the facing of the piston base or extension.
#[inline]
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

/// Set the facing of the piston base or extension.
#[inline]
pub fn set_face(metadata: &mut u8, face: Face) {
    *metadata &= !7;
    *metadata |= face as u8;
}
