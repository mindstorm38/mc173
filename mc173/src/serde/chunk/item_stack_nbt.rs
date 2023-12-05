//! NBT serialization and deserialization for [`ItemStack`] type.

use crate::item::ItemStack;

#[derive(Debug, serde::Serialize, serde::Deserialize)]
struct ItemStackNbt {
    id: u16,
    #[serde(rename = "Count")]
    size: i8,
    #[serde(rename = "Damage")]
    damage: u16,
}

pub fn deserialize<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<ItemStack, D::Error> {
    let nbt: ItemStackNbt = serde::Deserialize::deserialize(deserializer)?;
    Ok(ItemStack { 
        id: nbt.id, 
        size: nbt.size.max(0) as u16, 
        damage: nbt.damage,
    })
}

pub fn serialize<S: serde::Serializer>(value: &ItemStack, serializer: S) -> Result<S::Ok, S::Error> {
    serde::Serialize::serialize(&ItemStackNbt {
        id: value.id,
        size: value.size.min(i8::MAX as _) as i8,
        damage: value.damage,
    }, serializer)
}
