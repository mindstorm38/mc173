//! NBT serialization and deserialization for [`PaintingArt`] enumeration.

use crate::entity::PaintingArt;

pub fn from_nbt(id: &str) -> Option<PaintingArt> {
    Some(match id {
        "Kebab" => PaintingArt::Kebab,
        "Aztec" => PaintingArt::Aztec,
        "Alban" => PaintingArt::Alban,
        "Aztec2" => PaintingArt::Aztec2,
        "Bomb" => PaintingArt::Bomb,
        "Plant" => PaintingArt::Plant,
        "Wasteland" => PaintingArt::Wasteland,
        "Pool" => PaintingArt::Pool,
        "Courbet" => PaintingArt::Courbet,
        "Sea" => PaintingArt::Sea,
        "Sunset" => PaintingArt::Sunset,
        "Creebet" => PaintingArt::Creebet,
        "Wanderer" => PaintingArt::Wanderer,
        "Graham" => PaintingArt::Graham,
        "Match" => PaintingArt::Match,
        "Bust" => PaintingArt::Bust,
        "Stage" => PaintingArt::Stage,
        "Void" => PaintingArt::Void,
        "SkullAndRoses" => PaintingArt::SkullAndRoses,
        "Fighters" => PaintingArt::Fighters,
        "Pointer" => PaintingArt::Pointer,
        "Pigscene" => PaintingArt::Pigscene,
        "BurningSkull" => PaintingArt::BurningSkull,
        "Skeleton" => PaintingArt::Skeleton,
        "DonkeyKong" => PaintingArt::DonkeyKong,
        _ => return None
    })
}

pub fn to_nbt(art: PaintingArt) -> &'static str {
    match art {
        PaintingArt::Kebab => "Kebab",
        PaintingArt::Aztec => "Aztec",
        PaintingArt::Alban => "Alban",
        PaintingArt::Aztec2 => "Aztec2",
        PaintingArt::Bomb => "Bomb",
        PaintingArt::Plant => "Plant",
        PaintingArt::Wasteland => "Wasteland",
        PaintingArt::Pool => "Pool",
        PaintingArt::Courbet => "Courbet",
        PaintingArt::Sea => "Sea",
        PaintingArt::Sunset => "Sunset",
        PaintingArt::Creebet => "Creebet",
        PaintingArt::Wanderer => "Wanderer",
        PaintingArt::Graham => "Graham",
        PaintingArt::Match => "Match",
        PaintingArt::Bust => "Bust",
        PaintingArt::Stage => "Stage",
        PaintingArt::Void => "Void",
        PaintingArt::SkullAndRoses => "SkullAndRoses",
        PaintingArt::Fighters => "Fighters",
        PaintingArt::Pointer => "Pointer",
        PaintingArt::Pigscene => "Pigscene",
        PaintingArt::BurningSkull => "BurningSkull",
        PaintingArt::Skeleton => "Skeleton",
        PaintingArt::DonkeyKong => "DonkeyKong",
    }
}
