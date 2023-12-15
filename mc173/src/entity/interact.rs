//! Methods to interact with entities.

use glam::DVec3;

use crate::item;

use super::{Entity, BaseKind, Base, Living, LivingKind};


impl Entity {

    /// Attack the entity with the given item.
    pub fn hurt_with(&mut self, item: u16, bonus: u16, origin: Option<DVec3>) -> bool {
        
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

        self.hurt(damage, origin)

    }

    pub fn hurt(&mut self, damage: u16, origin: Option<DVec3>) -> bool {

        let Entity(base, base_kind) = self;

        if let BaseKind::Living(living, living_kind) = base_kind {
            hurt_living(base, living, living_kind, damage, origin)
        } else {
            false
        }

    }

}


/// Internal function to attack a living entity. This function returns true when the
/// entity has successfully received damages.
fn hurt_living(base: &mut Base, living: &mut Living, living_kind: &mut LivingKind, damage: u16, origin: Option<DVec3>) -> bool {

    if living.health == 0 {
        return false;
    }

    /// The hurt time when hit for the first time.
    /// PARITY: The Notchian impl doesn't actually use hurt time but another variable
    /// that have the exact same behavior, so we use hurt time here to be more consistent.
    const HURT_MAX_TIME: u16 = 20;

    if living.hurt_time > HURT_MAX_TIME / 2 {

        if damage <= living.hurt_damage {
            return false;
        }
        
        damage_living(base, living, living_kind, damage - living.hurt_damage);
        living.hurt_damage = damage;

    } else {

        damage_living(base, living, living_kind, damage);
        living.hurt_damage = damage;
        living.hurt_time = HURT_MAX_TIME;

        // TODO: Send damage status packet 2

        if let Some(origin) = origin {

            let mut dir = origin - base.pos;
            dir.y = 0.0; // We ignore verticle delta.

            while dir.length_squared() < 1.0e-4 {
                dir = DVec3 {
                    x: (base.rand.next_double() - base.rand.next_double()) * 0.01,
                    y: 0.0,
                    z: (base.rand.next_double() - base.rand.next_double()) * 0.01,
                }
            }

            base.update_knock_back(dir);

        }

    }

    true

}

/// Do the given amount of damages on the entity, no check is applied and damage are 
/// always applied to the entity.
fn damage_living(_base: &mut Base, living: &mut Living, _living_kind: &mut LivingKind, damage: u16) {
    
    living.health = living.health.saturating_sub(damage);

}
