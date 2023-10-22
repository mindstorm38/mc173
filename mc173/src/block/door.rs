//! Door block specific logic.


/// If the block is a door (iron/wood), get if it's in open state.
#[inline]
pub fn is_open(metadata: u8) -> bool {
    metadata & 4 != 0
}
