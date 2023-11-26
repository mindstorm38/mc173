//! Sapling block metadata functions.

use crate::tree::TreeKind;


/// Get the kind of tree for this sapling.
#[inline]
pub fn get_kind(metadata: u8) -> TreeKind {
    match metadata & 3 {
        0 | 3 => TreeKind::Oak,
        1 => TreeKind::Taiga,
        2 => TreeKind::Birch,
        _ => unreachable!()
    }
}

/// Set the face where the pumpkin is carved.
#[inline]
pub fn set_kind(metadata: &mut u8, kind: TreeKind) {
    *metadata &= !3;
    *metadata |= match kind {
        TreeKind::Oak |
        TreeKind::Taiga => 1,
        TreeKind::Birch => 2,
    };
}

/// Return true if the sapling is growing and will grow on the next random tick.
#[inline]
pub fn is_growing(metadata: u8) -> bool {
    metadata & 8 != 0
}

/// Set if a sapling is growing.
#[inline]
pub fn set_growing(metadata: &mut u8, growing: bool) {
    *metadata &= !8;
    *metadata |= (growing as u8) << 3;
}
