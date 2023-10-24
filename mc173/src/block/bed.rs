//! Bed special functions for metadata.

use crate::util::Face;


/// Get the facing of a bed from its metadata.
pub fn get_face(metadata: u8) -> Face {
    match metadata & 3 {
        0 => Face::PosZ,
        1 => Face::NegX,
        2 => Face::NegZ,
        3 => Face::PosX,
        _ => unreachable!()
    }
}

/// Return true if the bed is occupied.
pub fn is_occupied(metadata: u8) -> bool {
    metadata & 4 != 0
}

/// Return true if this bed's block is the head piece.
pub fn is_head(metadata: u8) -> bool {
    metadata & 8 != 0
}
