//! Block colliding behaviors for blocks.

use glam::{IVec3, DVec3};

use crate::util::{BoundingBox, Face};
use crate::world::World;
use crate::block;


const PIXEL: f64 = 1.0 / 16.0;
const PIXEL_2: f64 = 2.0 / 16.0;
const PIXEL_3: f64 = 3.0 / 16.0;


/// Get the colliding boxes for a block, the colliding box will be offset to the block's 
/// position as needed. Not to confuse with overlay boxes, which are just used to client
/// side placement rendering, and used server-side to compute ray tracing when using
/// items such as bucket.
pub fn iter_colliding_box<'w>(world: &'w World, pos: IVec3, id: u8, metadata: u8) -> impl Iterator<Item = BoundingBox> + 'w {
    BoundingBoxIter { world, pos, id, metadata, index: 0 }
}

/// Internal iterator implementation for bounding boxes of a block with metadata, we must
/// use an iterator because some blocks have multiple bounding boxes.
struct BoundingBoxIter<'w> {
    /// The world where the bounding box is iterated.
    /// TODO: This is for block entities.
    #[allow(unused)]
    world: &'w World,
    /// The block position in the world, the returned bounding box is offset by this.
    pos: IVec3,
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
                block::piston::get_face(metadata)?.extrude(0.0, 0.25)
            }
            (1, block::PISTON_EXT) => {
                // The extension rod second.
                let face = block::piston::get_face(metadata)?;
                face.extrude(6.0 / 16.0, 0.75) - face.delta().as_dvec3() * 0.25
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

        Some(bb.offset(self.pos.as_dvec3()))

    }

}

/// Get the overlay box of the block. The returned bounding box is offset by given block
/// position.
pub fn get_overlay_box(world: &World, pos: IVec3, id: u8, metadata: u8) -> Option<BoundingBox> {

    let _ = world;

    let bb = match id {
        block::BED => BoundingBox::new(0.0, 0.0, 0.0, 1.0, 9.0 / 16.0, 1.0),
        block::CAKE => BoundingBox::new((1 + metadata * 2) as f64 / 16.0, 0.0, PIXEL, 1.0 - PIXEL, 0.5, 1.0 - PIXEL),
        block::WOOD_DOOR | 
        block::IRON_DOOR => block::door::get_actual_face(metadata).extrude(0.0, PIXEL_3),
        block::LEVER => {
            let (face, _) = block::lever::get_face(metadata)?;
            if face == Face::NegY {
                face.extrude(0.25, 0.6)
            } else {
                face.extrude(5.0 / 16.0, 6.0 / 16.0).inflate(DVec3::new(0.0, PIXEL_2, 0.0))
            }
        }
        block::BUTTON => {
            let face = block::button::get_face(metadata)?;
            let active = block::button::is_active(metadata);
            face.extrude(0.6, if active { PIXEL } else { PIXEL_2 })
                .inflate(DVec3::new(0.0, -PIXEL, 0.0))
        }
        block::PISTON |
        block::STICKY_PISTON => {
            if block::piston::is_extended(metadata) {
                block::piston::get_face(metadata)?.extrude(0.0, 12.0 / 16.0)
            } else {
                BoundingBox::CUBE
            }
        }
        block::PISTON_EXT => block::piston::get_face(metadata)?.extrude(0.0, 0.25),
        block::PISTON_MOVING => return None,  // TODO: Use block entity.
        block::PORTAL => return None,  // TODO: Use surrounding portals to determine
        block::WOOD_PRESSURE_PLATE |
        block::STONE_PRESSURE_PLATE => {
            Face::NegY.extrude(PIXEL, if metadata == 1 { PIXEL / 2.0 } else { PIXEL })
        }
        block::RAIL |
        block::POWERED_RAIL |
        block::DETECTOR_RAIL => {
            // TODO: Use proper metadata functions when implementing rails.
            Face::NegY.extrude(0.0, if metadata >= 2 && metadata <= 5 { 10.0 / 16.0 } else { PIXEL_2 })
        }
        block::SIGN |
        block::WALL_SIGN => return None,  // TODO:
        block::SNOW => {
            let layers = metadata & 7;
            Face::NegY.extrude(0.0, 2.0 * (1.0 + layers as f64) / 16.0)
        }
        block::TRAPDOOR => {
            if block::trapdoor::is_open(metadata) {
                block::trapdoor::get_face(metadata).extrude(0.0, PIXEL_3)
            } else {
                Face::NegY.extrude(0.0, PIXEL_3)
            }
        }
        block::AIR => return None,
        _ => BoundingBox::CUBE,
    };

    Some(bb.offset(pos.as_dvec3()))

}
