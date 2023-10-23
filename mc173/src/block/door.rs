//! Door block specific logic.


/// If the block is a door (iron/wood), get if it's in open state.
pub fn is_open(metadata: u8) -> bool {
    metadata & 4 != 0
}

/// Return true if this door block is the upper part.
pub fn is_upper(metadata: u8) -> bool {
    metadata & 8 != 0
}
