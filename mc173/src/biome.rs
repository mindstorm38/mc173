//! Biome in beta 1.7.3 are a pure chunk generation thing and do not exists after.
//! The client seems to use the seed sent by the server to recompute foliage color.


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
