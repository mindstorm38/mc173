//! Block colliding behaviors for blocks.

use crate::util::{BoundingBox, Face};
use crate::world::World;
use crate::block;


/// Get the bounding box for a block, the bounding box will be offset to the block's 
/// position as needed. This function currently support at most 
pub fn iter_bounding_box<'w>(world: &'w World, id: u8, metadata: u8) -> impl Iterator<Item = BoundingBox> + 'w {
    BoundingBoxIter { world, id, metadata, index: 0 }
}

/// Internal iterator implementation for bounding boxes of a block with metadata, we must
/// use an iterator because some blocks have multiple bounding boxes.
struct BoundingBoxIter<'w> {
    /// The world where the bounding box is iterated.
    /// TODO: This is for block entities.
    #[allow(unused)]
    world: &'w World,
    /// The block id.
    id: u8,
    /// The block metadata.
    metadata: u8,
    /// The index of the bounding box, for example stairs have 2 bounding boxes.
    index: u8,
}

impl<'w> Iterator for BoundingBoxIter<'w> {

    type Item = BoundingBox;

    fn next(&mut self) -> Option<Self::Item> {

        const PIXEL: f64 = 1.0 / 16.0;
        const PIXEL_2: f64 = 2.0 / 16.0;
        const PIXEL_3: f64 = 3.0 / 16.0;

        let metadata = self.metadata;
        let index = self.index;
        self.index += 1;

        let bb = match (index, self.id) {
            (0, block::CACTUS) => BoundingBox::new(PIXEL, 0.0, PIXEL, 1.0 - PIXEL, 1.0, 1.0 - PIXEL),
            (0, block::CAKE) => BoundingBox::new((1 + metadata * 2) as f64 / 16.0, 0.0, PIXEL, 1.0 - PIXEL, 0.5, 1.0 - PIXEL),
            (0, block::FENCE) => BoundingBox::new(0.0, 0.0, 0.0, 1.0, 1.5, 1.0),
            (0, block::SOULSAND) => BoundingBox::new(0.0, 0.0, 0.0, 1.0, 1.0 - PIXEL_2, 1.0),
            (0, block::BED) => BoundingBox::new(0.0, 0.0, 0.0, 1.0, 9.0 / 16.0, 1.0),
            (0, block::REPEATER | block::REPEATER_LIT) => BoundingBox::new(0.0, 0.0, 0.0, 1.0, PIXEL_2, 1.0),
            (0, block::WOOD_DOOR | block::IRON_DOOR) => block::door::get_actual_face(metadata).extrude(0.0, PIXEL_3),
            (0, block::LADDER) => return block::ladder::get_face(metadata).map(|face| face.extrude(0.0, PIXEL_2)),
            (0, block::SNOW) => {
                let layers = metadata & 7;
                if layers >= 3 {
                    BoundingBox::new(0.0, 0.0, 0.0, 1.0, 0.5, 1.0)
                } else {
                    return None
                }
            }
            (0, block::TRAPDOOR) => {
                if block::trapdoor::is_open(metadata) {
                    block::trapdoor::get_face(metadata).extrude(0.0, PIXEL_3)
                } else {
                    Face::NegY.extrude(0.0, PIXEL_3)
                }
            }
            (0, block::PISTON_MOVING) => {
                return None;  // TODO: depends on tile entity!
            }
            (0, block::PISTON_EXT) => {
                // The extension plate first.
                return block::piston::get_face(metadata).map(|face| face.extrude(0.0, 0.25));
            }
            (1, block::PISTON_EXT) => {
                // The extension rod second.
                return block::piston::get_face(metadata).map(|face| face.extrude(6.0 / 16.0, 0.75) - face.delta().as_dvec3() * 0.25)
            }
            (0, block::WOOD_STAIR | block::COBBLESTONE_STAIR | block::SLAB) => {
                // Slab and stair bottom piece.
                BoundingBox::new(0.0, 0.0, 0.0, 1.0, 0.5, 1.0)
            }
            (1, block::WOOD_STAIR | block::COBBLESTONE_STAIR) => {
                // The stair top piece (the bottom)
                block::stair::get_face(metadata).extrude(0.0, 0.5)
            }
            (0, block::AIR |
                block::FIRE |
                block::DANDELION | block::POPPY |
                block::WHEAT |
                block::DEAD_BUSH |
                block::RED_MUSHROOM | block::BROWN_MUSHROOM |
                block::TALL_GRASS |
                block::SUGAR_CANES |
                block::WATER_MOVING | block::WATER_STILL |
                block::LAVA_MOVING | block::LAVA_STILL |
                block::PORTAL |
                block::WOOD_PRESSURE_PLATE |
                block::STONE_PRESSURE_PLATE |
                block::RAIL | block::POWERED_RAIL | block::DETECTOR_RAIL |
                block::REDSTONE |
                block::BUTTON |
                block::LEVER |
                block::SIGN | block::WALL_SIGN |
                block::TORCH | block::REDSTONE_TORCH | block::REDSTONE_TORCH_LIT |
                block::COBWEB) => return None,
            // All blocks have a cube bounding box by default.
            (0, _) => BoundingBox::CUBE,
            // After index 1, defaults to None in order to stop iterator.
            _ => return None
        };

        Some(bb)

    }

}
