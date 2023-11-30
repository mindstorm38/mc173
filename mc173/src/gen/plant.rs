//! Plants feature generation.

use glam::IVec3;

use crate::util::{JavaRandom, Face};
use crate::world::World;
use crate::block;

use super::FeatureGenerator;


/// A generator for flower patch.
pub struct FlowerGenerator {
    flower_id: u8,
}

impl FlowerGenerator {
    #[inline]
    pub fn new(flower_id: u8) -> Self {
        Self { flower_id, }
    }
}

impl FeatureGenerator for FlowerGenerator {

    fn generate(&mut self, world: &mut World, pos: IVec3, rand: &mut JavaRandom) -> bool {
        
        for _ in 0..64 {
            
            let place_pos = pos + IVec3 {
                x: rand.next_int_bounded(8) - rand.next_int_bounded(8),
                y: rand.next_int_bounded(4) - rand.next_int_bounded(4),
                z: rand.next_int_bounded(8) - rand.next_int_bounded(8),
            };

            // PARITY: Check parity of "canBlockStay"...
            if world.is_block_air(place_pos) && world.can_place_block(place_pos, Face::NegY, self.flower_id) {
                world.set_block(place_pos, self.flower_id, 0);
            }

        }

        true

    }

}


/// A generator for tall grass patch.
pub struct TallGrassGenerator {
    metadata: u8,
}

impl TallGrassGenerator {
    #[inline]
    pub fn new(metadata: u8) -> Self {
        Self { metadata, }
    }
}

impl FeatureGenerator for TallGrassGenerator {

    fn generate(&mut self, world: &mut World, mut pos: IVec3, rand: &mut JavaRandom) -> bool {
        
        while pos.y > 0 {
            if !matches!(world.get_block(pos), Some((block::AIR | block::LEAVES, _))) {
                break;
            }
            pos.y -= 1;
        }

        for _ in 0..128 {

            let place_pos = pos + IVec3 {
                x: rand.next_int_bounded(8) - rand.next_int_bounded(8),
                y: rand.next_int_bounded(4) - rand.next_int_bounded(4),
                z: rand.next_int_bounded(8) - rand.next_int_bounded(8),
            };

            // PARITY: Check parity of "canBlockStay"...
            if world.is_block_air(place_pos) && world.can_place_block(place_pos, Face::NegY, block::TALL_GRASS) {
                world.set_block(place_pos, block::TALL_GRASS, self.metadata);
            }

        }

        true

    }

}