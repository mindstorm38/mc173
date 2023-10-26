//! Common metadata functions.

use crate::util::Face;


/// Get the facing of this block, this common function works for some blocks that have
/// standard horizontal face metadata (2: -Z, 3: +Z, 4: -X, 5: +X).
pub fn get_horizontal_face(metadata: u8) -> Option<Face> {
    Some(match metadata {
        2 => Face::NegZ,
        3 => Face::PosZ,
        4 => Face::NegX,
        5 => Face::PosX,
        _ => return None
    })
}

/// Set the facing of this block.
pub fn set_horizontal_face(metadata: &mut u8, face: Face) {
    *metadata = face as u8;
}
