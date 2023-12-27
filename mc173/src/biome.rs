//! Biome in beta 1.7.3 are a pure chunk generation thing and do not exists after.
//! The client seems to use the seed sent by the server to recompute foliage color.


/// Possible biomes, only used server-side for natural mob spawning.
#[derive(Default, Debug, Clone, Copy, PartialEq, Eq)]
pub enum Biome {
    #[default]
    Void,
    RainForest,
    Swampland,
    SeasonalForest,
    Forest,
    Savanna,
    ShrubLand,
    Taiga,
    Desert,
    Plains,
    IceDesert,
    Tundra,
    Nether,
    Sky,
}

impl Biome {

    /// Return true if it is possible to rain in a chunk.
    #[inline]
    pub fn has_rain(self) -> bool {
        match self {
            Self::Desert |
            Self::IceDesert |
            Self::Nether |
            Self::Sky => false,
            _ => true
        }
    }

    /// Return true if this is snowing in the biome.
    #[inline]
    pub fn has_snow(self) -> bool {
        match self {
            Self::Taiga |
            Self::IceDesert |
            Self::Tundra => true,
            _ => false
        }
    }

}
