//! Methods to interact with entities.

use glam::DVec3;

use crate::item;

use super::{Entity, BaseKind};


impl Entity {

    /// Attack the entity with the given item.
    pub fn hurt_with(&mut self, item: u16, bonus: u16, origin: Option<DVec3>) {
        
        const DIAMOND_DAMAGE: u16 = 3;
        const IRON_DAMAGE: u16 = 2;
        const STONE_DAMAGE: u16 = 1;
        const WOOD_DAMAGE: u16 = 0;
        const GOLD_DAMAGE: u16 = 0;

        // Calculate the damage from the item.
        let damage = bonus + match item {
            // Sword
            item::DIAMOND_SWORD     => 4 + DIAMOND_DAMAGE * 2,
            item::IRON_SWORD        => 4 + IRON_DAMAGE * 2,
            item::STONE_SWORD       => 4 + STONE_DAMAGE * 2,
            item::WOOD_SWORD        => 4 + WOOD_DAMAGE * 2,
            item::GOLD_SWORD        => 4 + GOLD_DAMAGE * 2,
            // Axe
            item::DIAMOND_AXE       => 3 + DIAMOND_DAMAGE,
            item::IRON_AXE          => 3 + IRON_DAMAGE,
            item::STONE_AXE         => 3 + STONE_DAMAGE,
            item::WOOD_AXE          => 3 + WOOD_DAMAGE,
            item::GOLD_AXE          => 3 + GOLD_DAMAGE,
            // Pickaxe
            item::DIAMOND_PICKAXE   => 2 + DIAMOND_DAMAGE,
            item::IRON_PICKAXE      => 2 + IRON_DAMAGE,
            item::STONE_PICKAXE     => 2 + STONE_DAMAGE,
            item::WOOD_PICKAXE      => 2 + WOOD_DAMAGE,
            item::GOLD_PICKAXE      => 2 + GOLD_DAMAGE,
            // Shovel
            item::DIAMOND_SHOVEL    => 1 + DIAMOND_DAMAGE,
            item::IRON_SHOVEL       => 1 + IRON_DAMAGE,
            item::STONE_SHOVEL      => 1 + STONE_DAMAGE,
            item::WOOD_SHOVEL       => 1 + WOOD_DAMAGE,
            item::GOLD_SHOVEL       => 1 + GOLD_DAMAGE,
            // All other items make 1 damage.
            _ => 1,
        };

        self.hurt(damage, origin);

    }

    pub fn hurt(&mut self, damage: u16, origin: Option<DVec3>) {

        let Entity(_, base_kind) = self;

        if let BaseKind::Living(living, _) = base_kind {
            if living.hurt_damage == 0 {
                living.hurt_origin = origin;
            }
            living.hurt_damage = living.hurt_damage.max(damage);
        }

    }

}
