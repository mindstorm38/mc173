//! Stair block metadata functions.

use crate::util::Face;


/// Get the face where the stair leads to.
pub fn get_face(metadata: u8) -> Face {
    match metadata & 3 {
        0 => Face::PosX,
        1 => Face::NegX,
        2 => Face::PosZ,
        3 => Face::NegZ,
        _ => unreachable!()
    }
}
