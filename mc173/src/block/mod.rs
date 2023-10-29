//! Block enumeration and behaviors.

use crate::item::Item;

// Block behaviors.
pub mod material;
pub mod colliding;
pub mod notifying;
pub mod powering;
pub mod dropping;
pub mod breaking;
pub mod ticking;
pub mod placing;
pub mod using;

// Block specific functions for their metadata.
pub mod common;
pub mod trapdoor;
pub mod repeater;
pub mod pumpkin;
pub mod button;
pub mod ladder;
pub mod piston;
pub mod lever;
pub mod stair;
pub mod torch;
pub mod fluid;
pub mod door;
pub mod bed;


/// Internal macro to easily define blocks registry.
macro_rules! blocks {
    (
        $($name:ident / $id:literal : $init:expr),* $(,)?
    ) => {

        static BLOCKS: [Block; 256] = {

            const DEFAULT: Block = Block::new("", Material::Air, 0.0);

            let mut arr = [DEFAULT; 256];
            $(arr[$id as usize] = $init;)*
            arr

        };

        $(pub const $name: u8 = $id;)*

    };
}

blocks! {
    AIR/0:              Block::new("air", Material::Air, 0.0),
    STONE/1:            Block::new("stone", Material::Rock, 30.0),
    GRASS/2:            Block::new("grass", Material::Grass, 0.0),
    DIRT/3:             Block::new("dirt", Material::Ground, 0.0),
    COBBLESTONE/4:      Block::new("cobblestone", Material::Rock, 30.0),
    WOOD/5:             Block::new("wood", Material::Wood, 15.0),
    SAPLING/6:          Block::new("sapling", Material::Plant, 0.0),
    BEDROCK/7:          Block::new("bedrock", Material::Rock, 18000000.0),
    WATER_MOVING/8:     Block::new("water_moving", Material::Water, 0.0).set_light_opacity(3),
    WATER_STILL/9:      Block::new("water_still", Material::Water, 0.0).set_light_opacity(3),
    LAVA_MOVING/10:     Block::new("lava_moving", Material::Lava, 0.0).set_light_emission(15),
    LAVA_STILL/11:      Block::new("lava_still", Material::Lava, 0.0).set_light_emission(15),
    SAND/12:            Block::new("sand", Material::Sand, 0.0),
    GRAVEL/13:          Block::new("gravel", Material::Sand,0.0),
    GOLD_ORE/14:        Block::new("gold_ore", Material::Rock, 15.0),
    IRON_ORE/15:        Block::new("iron_ore", Material::Rock, 15.0),
    COAL_ORE/16:        Block::new("coal_ore", Material::Rock, 15.0),
    LOG/17:             Block::new("log", Material::Wood, 0.0),
    LEAVES/18:          Block::new("leaves", Material::Leaves, 0.0),
    SPONGE/19:          Block::new("sponge", Material::Sponge, 0.0),
    GLASS/20:           Block::new("glass", Material::Glass, 0.0),
    LAPIS_ORE/21:       Block::new("lapis_ore", Material::Rock, 15.0),
    LAPIS_BLOCK/22:     Block::new("lapis_block", Material::Rock, 15.0),
    DISPENSER/23:       Block::new("dispenser", Material::Rock, 0.0),
    SANDSTONE/24:       Block::new("sandstone", Material::Rock, 0.0),
    NOTE_BLOCK/25:      Block::new("note_block", Material::Wood, 0.0),
    BED/26:             Block::new("bed", Material::Cloth, 0.0),
    POWERED_RAIL/27:    Block::new("powered_rail", Material::Circuit, 0.0),
    DETECTOR_RAIL/28:   Block::new("detector_rail", Material::Circuit, 0.0),
    STICKY_PISTON/29:   Block::new("sticky_piston", Material::Piston, 0.0),
    COBWEB/30:          Block::new("cobweb", Material::Cobweb, 0.0),
    TALL_GRASS/31:      Block::new("tall_grass", Material::Plant, 0.0),
    DEAD_BUSH/32:       Block::new("dead_bush", Material::Plant, 0.0),
    PISTON/33:          Block::new("piston", Material::Piston, 0.0),
    PISTON_EXT/34:      Block::new("piston_ext", Material::Piston, 0.0),
    WOOL/35:            Block::new("wool", Material::Cloth, 0.0),
    PISTON_MOVING/36:   Block::new("piston_moving", Material::Piston, 0.0),
    DANDELION/37:       Block::new("dandelion", Material::Plant, 0.0),
    POPPY/38:           Block::new("poppy", Material::Plant, 0.0),
    BROWN_MUSHROOM/39:  Block::new("brown_mushroom", Material::Plant, 0.0),
    RED_MUSHROOM/40:    Block::new("red_mushroom", Material::Plant, 0.0),
    GOLD_BLOCK/41:      Block::new("gold_block", Material::Iron, 30.0),
    IRON_BLOCK/42:      Block::new("iron_block", Material::Iron, 30.0),
    DOUBLE_SLAB/43:     Block::new("double_slab", Material::Rock,30.0),
    SLAB/44:            Block::new("slab", Material::Rock, 30.0),
    BRICK/45:           Block::new("brick", Material::Rock, 30.0),
    TNT/46:             Block::new("tnt", Material::Tnt, 0.0),
    BOOKSHELF/47:       Block::new("bookshelf", Material::Wood, 0.0),
    MOSSY_COBBLESTONE/48: Block::new("mossy_cobblestone", Material::Rock, 30.0),
    OBSIDIAN/49:        Block::new("obsidian", Material::Rock, 6000.0),
    TORCH/50:           Block::new("torch", Material::Circuit, 0.0).set_light_emission(14),
    FIRE/51:            Block::new("fire", Material::Fire, 0.0).set_light_emission(15),
    SPAWNER/52:         Block::new("spawner", Material::Rock, 0.0),
    WOOD_STAIR/53:      Block::new("wood_stair", Material::Wood, 15.0),
    CHEST/54:           Block::new("chest", Material::Wood, 0.0),
    REDSTONE/55:        Block::new("redstone", Material::Circuit, 0.0),
    DIAMOND_ORE/56:     Block::new("diamond_ore", Material::Rock, 15.0),
    DIAMOND_BLOCK/57:   Block::new("diamond_block", Material::Iron, 30.0),
    CRAFTING_TABLE/58:  Block::new("crafting_table", Material::Wood, 0.0),
    WHEAT/59:           Block::new("wheat", Material::Plant, 0.0),
    FARMLAND/60:        Block::new("farmland", Material::Ground, 0.0),
    FURNACE/61:         Block::new("furnace", Material::Rock, 0.0),
    FURNACE_LIT/62:     Block::new("furnace_lit", Material::Rock, 0.0).set_light_emission(14),
    SIGN/63:            Block::new("sign", Material::Wood, 0.0),
    WOOD_DOOR/64:       Block::new("wood_door", Material::Wood, 0.0),
    LADDER/65:          Block::new("ladder", Material::Circuit, 0.0),
    RAIL/66:            Block::new("rail", Material::Circuit, 0.0),
    COBBLESTONE_STAIR/67: Block::new("cobblestone_stair", Material::Rock, 30.0),
    WALL_SIGN/68:       Block::new("wall_sign", Material::Wood, 0.0),
    LEVER/69:           Block::new("lever", Material::Circuit, 0.0),
    STONE_PRESSURE_PLATE/70: Block::new("stone_pressure_plate", Material::Rock, 0.0),
    IRON_DOOR/71:       Block::new("iron_door", Material::Iron, 15.0),
    WOOD_PRESSURE_PLATE/72: Block::new("wood_pressure_plate", Material::Wood, 0.0),
    REDSTONE_ORE/73:    Block::new("redstone_ore", Material::Rock, 15.0),
    REDSTONE_ORE_LIT/74: Block::new("redstone_ore_lit", Material::Rock, 15.0).set_light_emission(9),
    REDSTONE_TORCH/75:  Block::new("redstone_torch", Material::Circuit, 0.0),
    REDSTONE_TORCH_LIT/76:  Block::new("redstone_torch_lit", Material::Circuit, 0.0).set_light_emission(7),
    BUTTON/77:          Block::new("button", Material::Circuit, 0.0),
    SNOW/78:            Block::new("snow", Material::Snow, 0.0),
    ICE/79:             Block::new("ice", Material::Ice, 0.0).set_light_opacity(3).set_slipperiness(0.98),
    SNOW_BLOCK/80:      Block::new("snow_block", Material::SnowBlock, 0.0),
    CACTUS/81:          Block::new("cactus", Material::Cactus, 0.0),
    CLAY/82:            Block::new("clay", Material::Clay, 0.0),
    SUGAR_CANES/83:     Block::new("sugar_canes", Material::Plant, 0.0),
    JUKEBOX/84:         Block::new("jukebox", Material::Wood, 30.0),
    FENCE/85:           Block::new("fence", Material::Wood, 5.0),
    PUMPKIN/86:         Block::new("pumpkin", Material::Pumpkin, 0.0),
    NETHERRACK/87:      Block::new("netherrack", Material::Rock, 0.0),
    SOULSAND/88:        Block::new("soulsand", Material::Sand, 0.0),
    GLOWSTONE/89:       Block::new("glowstone", Material::Rock, 0.0).set_light_emission(15),
    PORTAL/90:          Block::new("portal", Material::Portal, 0.0).set_light_emission(15),
    PUMPKIN_LIT/91:     Block::new("pumpkin_lit", Material::Pumpkin, 0.0),
    CAKE/92:            Block::new("cake", Material::Cake, 0.0).set_light_opacity(0),
    REPEATER/93:        Block::new("repeater", Material::Circuit, 0.0),
    REPEATER_LIT/94:    Block::new("repeater_lit", Material::Circuit, 0.0).set_light_emission(9),
    LOCKED_CHEST/95:    Block::new("locked_chest", Material::Wood, 0.0).set_light_emission(15),
    TRAPDOOR/96:        Block::new("trapdoor", Material::Wood, 0.0).set_light_opacity(0),
}


/// Get a block from its numeric id.
pub fn from_id(id: u8) -> &'static Block {
    &BLOCKS[id as usize]
}

/// Find a block id from its name.
pub fn from_name(name: &str) -> Option<u8> {
    BLOCKS.iter().enumerate()
        .find(|(_, item)| item.name == name)
        .map(|(i, _)| i as u8)
}


/// This structure describe a block.
#[derive(Debug, Clone, Copy)]
pub struct Block {
    /// The name of the block, used for debug purpose.
    pub name: &'static str,
    /// The block material defining its common properties.
    pub material: Material,
    /// Block resistance to explosions.
    /// TODO: Move to specific module.
    pub resistance: f32,
    /// The block slipperiness for entities.
    pub slipperiness: f32,
    /// Opacity to light.
    /// TODO: Move to specific module.
    pub light_opacity: u8,
    /// Light emission.
    /// TODO: Move to specific module.
    pub light_emission: u8,
    /// The item corresponding to this block.
    pub item: Item,
}

impl Block {

    pub const fn new(name: &'static str, material: Material, resistance: f32) -> Self {
        Self {
            name,
            material,
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
    Cobweb,
    Piston,
}

impl Material {

    pub fn is_solid(self) -> bool {
        !matches!(self, 
            Self::Air |
            Self::Water |
            Self::Lava |
            Self::Plant |
            Self::Snow |
            Self::Circuit |
            Self::Portal |
            Self::Fire)
    }

    pub fn is_fluid(self) -> bool {
        matches!(self, Self::Water | Self::Lava)
    }

    pub fn is_translucent(self) -> bool {
        matches!(self, 
            Self::Leaves | 
            Self::Glass | 
            Self::Tnt | 
            Self::Ice | 
            Self::Snow | 
            Self::Cactus)
    }

    pub fn is_opaque(self) -> bool {
        !self.is_translucent() && self.is_solid()
    }

    pub fn is_replaceable(self) -> bool {
        matches!(self, 
            Self::Air |
            Self::Water |
            Self::Lava |
            Self::Snow |
            Self::Fire)
    }

    pub fn is_breakable_by_default(self) -> bool {
        !matches!(self,
            Self::Rock |
            Self::Iron |
            Self::Snow |
            Self::SnowBlock |
            Self::Cobweb
        )
    }

}
