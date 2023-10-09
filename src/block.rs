//! Block enumeration and behaviors.

/// Internal macro to easily define blocks registry.
macro_rules! blocks {
    (
        $($name:ident / $id:literal : $init:expr),* $(,)?
    ) => {

        pub static BLOCKS: [Block; 256] = {
            let mut arr = [Block::new("undefined", Material::Air, 0.0, 0.0); 256];
            $(arr[$id as usize] = $init;)*
            arr
        };

        $(pub const $name: u8 = $id;)*

    };
}

blocks! {
    AIR/0:          Block::new("air", Material::Air, 0.0, 0.0),
    STONE/1:        Block::new("stone", Material::Rock, 1.5, 30.0),
    GRASS/2:        Block::new("grass", Material::Grass, 0.6, 0.0),
    DIRT/3:         Block::new("dirt", Material::Ground, 0.5, 0.0),
    COBBLESTONE/4:  Block::new("cobblestone", Material::Rock, 2.0, 30.0),
    WOOD/5:         Block::new("wood", Material::Wood, 2.0, 15.0),
    SAPLING/6:      Block::new("sapling", Material::Plant, 0.0, 0.0),
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
        }
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
