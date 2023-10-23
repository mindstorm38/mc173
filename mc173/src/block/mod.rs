//! Block enumeration and behaviors.

use glam::IVec3;

use crate::util::bb::BoundingBox;
use crate::item::Item;

pub mod drop;

pub mod fluid;
pub mod door;
pub mod bed;


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
    WATER_MOVING/8:     Block::new("water_moving", Material::Water, 100.0, 0.0).set_light_opacity(3),
    WATER_STILL/9:      Block::new("water_moving", Material::Water, 100.0, 0.0).set_light_opacity(3),
    LAVA_MOVING/10:     Block::new("lava_moving", Material::Lava, 100.0, 0.0).set_light_emission(15),
    LAVA_STILL/11:      Block::new("lava_moving", Material::Lava, 100.0, 0.0).set_light_emission(15),
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
    DOUBLE_SLAB/43:     Block::new("double_slab", Material::Rock, 2.0, 30.0),
    SLAB/44:            Block::new("slab", Material::Rock, 2.0, 30.0),
    BRICK/45:           Block::new("brick", Material::Rock, 2.0, 30.0),
    TNT/46:             Block::new("tnt", Material::Tnt, 0.0, 0.0),
    BOOKSHELF/47:       Block::new("bookshelf", Material::Wood, 1.5, 0.0),
    MOSSY_COBBLESTONE/48: Block::new("mossy_cobblestone", Material::Rock, 2.0, 30.0),
    OBSIDIAN/49:        Block::new("obsidian", Material::Rock, 10.0, 6000.0),
    TORCH/50:           Block::new("torch", Material::Circuit, 0.0, 0.0).set_light_emission(14),
    FIRE/51:            Block::new("fire", Material::Fire, 0.0, 0.0).set_light_emission(15),
    SPAWNER/52:         Block::new("spawner", Material::Rock, 5.0, 0.0),
    WOOD_STAIR/53:      Block::new("wood_stair", Material::Wood, 2.0, 15.0),
    CHEST/54:           Block::new("chest", Material::Wood, 2.5, 0.0),
    REDSTONE/55:        Block::new("redstone", Material::Circuit, 0.0, 0.0),
    DIAMOND_ORE/56:     Block::new("diamond_ore", Material::Rock, 3.0, 15.0),
    DIAMOND_BLOCK/57:   Block::new("diamond_block", Material::Iron, 5.0, 30.0),
    CRAFTING_TABLE/58:  Block::new("crafting_table", Material::Wood, 2.5, 0.0),
    WHEAT/59:           Block::new("wheat", Material::Plant, 0.0, 0.0),
    FARMLAND/60:        Block::new("farmland", Material::Ground, 0.6, 0.0),
    FURNACE/61:         Block::new("furnace", Material::Rock, 3.5, 0.0),
    FURNACE_LIT/62:     Block::new("furnace_lit", Material::Rock, 3.5, 0.0).set_light_emission(14),
    SIGN/63:            Block::new("sign", Material::Wood, 1.0, 0.0),
    WOOD_DOOR/64:       Block::new("wood_door", Material::Wood, 3.0, 0.0),
    LADDER/65:          Block::new("ladder", Material::Circuit, 0.4, 0.0),
    RAIL/66:            Block::new("rail", Material::Circuit, 0.7, 0.0),
    COBBLESTONE_STAIR/67: Block::new("cobblestone_stair", Material::Rock, 2.0, 30.0),
    WALL_SIGN/68:       Block::new("wall_sign", Material::Wood, 1.0, 0.0),
    LEVER/69:           Block::new("lever", Material::Circuit, 0.5, 0.0),
    STONE_PRESSURE_PLATE/70: Block::new("stone_pressure_plate", Material::Rock, 0.5, 0.0),
    IRON_DOOR/71:       Block::new("iron_door", Material::Iron, 5.0, 15.0),
    WOOD_PRESSURE_PLATE/72: Block::new("wood_pressure_plate", Material::Wood, 0.5, 0.0),
    REDSTONE_ORE/73:    Block::new("redstone_ore", Material::Rock, 3.0, 15.0),
    REDSTONE_ORE_GLOWING/74: Block::new("redstone_ore", Material::Rock, 3.0, 15.0).set_light_emission(9),
    REDSTONE_TORCH/75:  Block::new("redstone_torch", Material::Circuit, 0.0, 0.0),
    REDSTONE_TORCH_LIT/76:  Block::new("redstone_torch_lit", Material::Circuit, 0.0, 0.0).set_light_emission(7),
    BUTTON/77:          Block::new("button", Material::Circuit, 0.5, 0.0),
    SNOW/78:            Block::new("snow", Material::Snow, 0.1, 0.0),
    ICE/79:             Block::new("ice", Material::Ice, 0.5, 0.0).set_light_opacity(3).set_slipperiness(0.98),
    SNOW_BLOCK/80:      Block::new("snow_block", Material::SnowBlock, 0.2, 0.0),
    CACTUS/81:          Block::new("cactus", Material::Cactus, 0.4, 0.0),
    CLAY/82:            Block::new("clay", Material::Clay, 0.6, 0.0),
    SUGAR_CANES/83:     Block::new("sugar_canes", Material::Plant, 0.0, 0.0),
    JUKEBOX/84:         Block::new("jukebox", Material::Wood, 2.0, 30.0),
    FENCE/85:           Block::new("fence", Material::Wood, 2.0, 5.0),
    PUMPKIN/86:         Block::new("pumpkin", Material::Pumpkin, 1.0, 0.0),
    NETHERRACK/87:      Block::new("netherrack", Material::Rock, 0.4, 0.0),
    SOULSAND/88:        Block::new("soulsand", Material::Sand, 0.5, 0.0),
    GLOWSTONE/89:       Block::new("glowstone", Material::Rock, 0.3, 0.0).set_light_emission(15),
    PORTAL/90:          Block::new("portal", Material::Portal, -1.0, 0.0).set_light_emission(15),
    PUMPKIN_LIT/91:     Block::new("pumpkin_lit", Material::Pumpkin, 1.0, 0.0),
    CAKE/92:            Block::new("cake", Material::Cake, 0.5, 0.0).set_light_opacity(0),
    REPEATER/93:        Block::new("repeater", Material::Circuit, 0.0, 0.0),
    REPEATER_LIT/94:    Block::new("repeater_lit", Material::Circuit, 0.0, 0.0).set_light_emission(9),
    LOCKED_CHEST/95:    Block::new("locked_chest", Material::Wood, 0.0, 0.0).set_light_emission(15),
    TRAPDOOR/96:        Block::new("trapdoor", Material::Wood, 3.0, 0.0).set_light_opacity(0),
}


/// Get a block from its numeric id.
pub fn from_id(id: u8) -> &'static Block {
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
    /// The item corresponding to this block.
    pub item: Item,
    /// This function is used to get the bounding box list of this block, given its 
    /// metadata. By default, this function just return a full block, the bounding box
    /// also needs to have its origin at 0/0/0, it will be offset when doing computations.
    pub fn_bounding_boxes: fn(u8, u8) -> &'static [BoundingBox],
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
            item: Item {
                name,
                block: true,
                max_stack_size: 64,
                max_damage: 0,
            },
            fn_bounding_boxes: |_, _| &[BoundingBox::CUBE],
        }
    }

    const fn set_slipperiness(mut self, slipperiness: f32) -> Self {
        self.slipperiness = slipperiness;
        self
    }

    const fn set_light_opacity(mut self, opacity: u8) -> Self {
        self.light_opacity = opacity;
        self
    }

    const fn set_light_emission(mut self, light: u8) -> Self {
        self.light_emission = light;
        self
    }
    
    const fn set_no_collide(self) -> Self {
        self.set_fn_bounding_boxes(|_, _| &[])
    }

    const fn set_fn_bounding_boxes(mut self, func: fn(u8, u8) -> &'static [BoundingBox]) -> Self {
        self.fn_bounding_boxes = func;
        self
    }

    /// Get bounding boxes for this block and given metadata.
    #[inline]
    pub fn bounding_boxes(&self, id: u8, metadata: u8) -> &'static [BoundingBox] {
        (self.fn_bounding_boxes)(id, metadata)
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
    SnowBlock,
    Cactus,
    Clay,
    Pumpkin,
    Portal,
    Cake,
    Web,
    Piston,
}

impl Material {

    pub fn is_solid(self) -> bool {
        !matches!(self, 
            Material::Air |
            Material::Water |
            Material::Lava |
            Material::Plant |
            Material::Snow |
            Material::Circuit |
            Material::Portal |
            Material::Fire
        )
    }

    pub fn is_fluid(self) -> bool {
        matches!(self, Material::Water | Material::Lava)
    }

}


/// Represent a block's face.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Face {
    NegY,
    PosY,
    NegZ,
    PosZ,
    NegX,
    PosX,
}

impl Face {

    /// Get the delta vector for this face.
    pub fn delta(self) -> IVec3 {
        match self {
            Face::NegY => IVec3::NEG_Y,
            Face::PosY => IVec3::Y,
            Face::NegZ => IVec3::NEG_Z,
            Face::PosZ => IVec3::Z,
            Face::NegX => IVec3::NEG_X,
            Face::PosX => IVec3::X,
        }
    }

}