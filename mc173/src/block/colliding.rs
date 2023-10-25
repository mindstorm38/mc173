//! Block colliding behaviors for blocks.

use crate::util::BoundingBox;
use crate::block;


/// Constant for the world size of a pixel with 16x16 textures.
const PIXEL: f64 = 1.0 / 16.0;

/// Get the bounding box for a block, the bounding box will be offset to the block's 
/// position as needed.
pub fn get_bounding_box(id: u8, metadata: u8, dst: &mut [BoundingBox]) -> usize {
    match id {
        block::BUTTON => 0,
        block::CACTUS => write_box(dst, [BoundingBox::new(PIXEL, 0.0, PIXEL, 1.0 - PIXEL, 1.0, 1.0 - PIXEL)]),
        block::CAKE => write_box(dst, [BoundingBox::new((1 + metadata * 2) as f64 / 16.0, 0.0, PIXEL, 1.0 - PIXEL, 0.5, 1.0 - PIXEL)]),
        block::WOOD_DOOR |
        block::IRON_DOOR => write_box(dst, [block::door::get_actual_face(metadata).extrude(3.0 / 16.0)]),
        _ => write_box(dst, [BoundingBox::CUBE])
    }
}

/// Internal utility to reduce boilerplate of box insertion.
#[inline(always)]
fn write_box<const LEN: usize>(dst: &mut [BoundingBox], src: [BoundingBox; LEN]) -> usize {
    dst.copy_from_slice(&src);
    src.len()
}
