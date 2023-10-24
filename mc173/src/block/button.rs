//! Button special functions for metadata.

use super::Face;


/// The the face the button is connected to. Note that `PosY` is not possible.
#[inline]
pub fn get_face(metadata: u8) -> Face {
    match metadata & 7 {
        1 => Face::NegX,
        2 => Face::PosX,
        3 => Face::NegZ,
        4 => Face::PosZ,
        _ => Face::NegY,
    }
}

/// Return true if the button is currently active.
#[inline]
pub fn is_active(metadata: u8) -> bool {
    metadata & 8 != 0
}

/// Set the button active or not.
#[inline]
pub fn set_active(metadata: &mut u8, active: bool) {
    *metadata &= !8;
    *metadata |= (active as u8) << 3;
}
