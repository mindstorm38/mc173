//! Block enumeration and behaviors.

use crate::util::bb::BoundingBox;


/// Internal macro to easily define blocks registry.
macro_rules! blocks {
    (
        $($name:ident / $id:literal : $init:expr),* $(,)?
    ) => {

        static BLOCKS: [Block; 256] = {

            const DEFAULT: Block = Block::new("undefined", Material::Air, 0.0, 0.0)
                .set_no_collide();

            let mut arr = [DEFAULT; 256];
            $(arr[$id as usize] = $init;)*
            arr

        };

        $(pub const $name: u8 = $id;)*

    };
}

blocks! {
    AIR/0:              Block::new("air", Material::Air, 0.0, 0.0).set_no_collide(),
    STONE/1:            Block::new("stone", Material::Rock, 1.5, 30.0),
    GRASS/2:            Block::new("grass", Material::Grass, 0.6, 0.0),
    DIRT/3:             Block::new("dirt", Material::Ground, 0.5, 0.0),
    COBBLESTONE/4:      Block::new("cobblestone", Material::Rock, 2.0, 30.0),
    WOOD/5:             Block::new("wood", Material::Wood, 2.0, 15.0),
    SAPLING/6:          Block::new("sapling", Material::Plant, 0.0, 0.0),
    BEDROCK/7:          Block::new("bedrock", Material::Rock, -1.0, 18000000.0),
    WATER_MOVING/8:     Block::new("water_moving", Material::Water, 100.0, 0.0),
    WATER_STILL/9:      Block::new("water_moving", Material::Water, 100.0, 0.0),
    LAVA_MOVING/10:     Block::new("lava_moving", Material::Lava, 100.0, 0.0),
    LAVA_STILL/11:      Block::new("lava_moving", Material::Lava, 100.0, 0.0),
    SAND/12:            Block::new("sand", Material::Sand, 0.5, 0.0),
    GRAVEL/13:          Block::new("gravel", Material::Sand, 0.6, 0.0),
    GOLD_ORE/14:        Block::new("gold_ore", Material::Rock, 3.0, 15.0),
    IRON_ORE/15:        Block::new("iron_ore", Material::Rock, 3.0, 15.0),
    COAL_ORE/16:        Block::new("coal_ore", Material::Rock, 3.0, 15.0),
    LOG/17:             Block::new("log", Material::Wood, 2.0, 0.0),
    LEAVES/18:          Block::new("leaves", Material::Leaves, 0.2, 0.0),
    SPONGE/19:          Block::new("sponge", Material::Sponge, 0.6, 0.0),
    GLASS/20:           Block::new("glass", Material::Glass, 0.3, 0.0),
    LAPIS_ORE/21:       Block::new("lapis_ore", Material::Rock, 3.0, 15.0),
    LAPIS_BLOCK/22:     Block::new("lapis_block", Material::Rock, 3.0, 15.0),
    DISPENSER/23:       Block::new("dispenser", Material::Rock, 3.5, 0.0),
    SANDSTONE/24:       Block::new("sandstone", Material::Rock, 0.8, 0.0),
    NOTE_BLOCK/25:      Block::new("note_block", Material::Wood, 0.8, 0.0),
    BED/26:             Block::new("bed", Material::Cloth, 0.2, 0.0),
    POWERED_RAIL/27:    Block::new("powered_rail", Material::Circuit, 0.7, 0.0),
    DETECTOR_RAIL/28:   Block::new("detector_rail", Material::Circuit, 0.7, 0.0),
    STICKY_PISTON/29:   Block::new("sticky_piston", Material::Piston, 0.5, 0.0),
    COBWEB/30:          Block::new("cobweb", Material::Web, 4.0, 0.0),
    TALL_GRASS/31:      Block::new("tall_grass", Material::Plant, 0.0, 0.0),
    DEAD_BUSH/32:       Block::new("dead_bush", Material::Plant, 0.0, 0.0),
    PISTON/33:          Block::new("piston", Material::Piston, 0.5, 0.0),
    PISTON_EXT/34:      Block::new("piston_ext", Material::Piston, 0.5, 0.0),
    WOOL/35:            Block::new("wool", Material::Cloth, 0.8, 0.0),
    PISTON_MOVING/36:   Block::new("piston_ext", Material::Piston, -1.0, 0.0),
    DANDELION/37:       Block::new("dandelion", Material::Plant, 0.0, 0.0),
    POPPY/38:           Block::new("poppy", Material::Plant, 0.0, 0.0),
    BROWN_MUSHROOM/39:  Block::new("brown_mushroom", Material::Plant, 0.0, 0.0),
    RED_MUSHROOM/40:    Block::new("red_mushroom", Material::Plant, 0.0, 0.0),
    GOLD_BLOCK/41:      Block::new("gold_block", Material::Iron, 3.0, 30.0),
    IRON_BLOCK/42:      Block::new("iron_block", Material::Iron, 5.0, 30.0),
    STAIR/43:           Block::new("stair", Material::Rock, 2.0, 30.0),
    SLAB/44:            Block::new("slab", Material::Rock, 2.0, 30.0),
    BRICK/45:           Block::new("brick", Material::Rock, 2.0, 30.0),
    TNT/46:             Block::new("tnt", Material::Tnt, 0.0, 0.0),
    BOOKSHELF/47:       Block::new("bookshelf", Material::Wood, 1.5, 0.0),
}

/// Get a block from its numeric id.
pub fn block_from_id(id: u8) -> &'static Block {
    &BLOCKS[id as usize]
}


/// This structure describe a block.
#[derive(Debug, Clone, Copy)]
pub struct Block {
    /// The name of the block, used for debug purpose.
    pub name: &'static str,
    /// The block material defining its common properties.
    pub material: Material,
    /// Block hardness for mining.
    pub hardness: f32,
    /// Block resistance to explosions.
    pub resistance: f32,
    /// The block slipperiness for entities.
    pub slipperiness: f32,
    /// Opacity to light.
    pub light_opacity: u8,
    /// Light emission.
    pub light_emission: u8,
    /// This function is used to get the bounding box list of this block, given its 
    /// metadata. By default, this function just return a full block, the bounding box
    /// also needs to have its origin at 0/0/0, it will be offset when doing computations.
    pub fn_bounding_boxes: fn(u8) -> &'static [BoundingBox],
}

impl Block {

    pub const fn new(name: &'static str, material: Material, hardness: f32, resistance: f32) -> Self {
        Self {
            name,
            material,
            hardness,
            resistance,
            slipperiness: 0.6,
            light_opacity: 255,
            light_emission: 0,
            fn_bounding_boxes: |_| &[BoundingBox::CUBE],
        }
    }
    
    const fn set_no_collide(self) -> Self {
        self.set_fn_bounding_boxes(|_| &[])
    }

    const fn set_fn_bounding_boxes(mut self, func: fn(u8) -> &'static [BoundingBox]) -> Self {
        self.fn_bounding_boxes = func;
        self
    }

    /// Get bounding boxes for this block and given metadata.
    #[inline]
    pub fn bounding_boxes(&self, metadata: u8) -> &'static [BoundingBox] {
        (self.fn_bounding_boxes)(metadata)
    }

}


/// Common block properties are defined through materials.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Material {
    Air,
    Grass,
    Ground,
    Wood,
    Rock,
    Iron,
    Water,
    Lava,
    Leaves,
    Plant,
    Sponge,
    Cloth,
    Fire,
    Sand,
    Circuit,
    Glass,
    Tnt,
    Wug,
    Ice,
    Snow,
    BuiltSnow,
    Cactus,
    Clay,
    Pumpkin,
    Portal,
    Cake,
    Web,
    Piston,
}
