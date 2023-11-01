//! Player entity implementation.

use glam::DVec3;

use crate::world::{World, Event};

use super::{PlayerEntity, Size, Entity};


impl PlayerEntity {

    /// Tick the player entity.
    pub fn tick_player(&mut self, world: &mut World) {
        
        self.tick_living(world, Size::new(0.6, 1.8), |_, _| {});
        
        // Player is manually moved from external logic, we still need to update the 
        // bounding box to account for the new position.
        self.update_bounding_box_from_pos();
        
        let main_inv = &mut self.kind.kind.main_inv;

        // First check immutable item if it's possible to pickup, if possible we append
        // them to consumed items and apply the change just after.
        let mut consumed_items = Vec::new();
        for (entity, _) in world.iter_entities_colliding(self.data.bb.inflate(DVec3::new(1.0, 0.0, 1.0))) {
            if let Entity::Item(base) = entity {
                if base.kind.frozen_ticks == 0 {
                    // Add the pickup item to the main inventory.
                    let picked_item = base.kind.stack;
                    let consumed_size = main_inv.add_stack(picked_item);
                    if consumed_size != 0 {
                        consumed_items.push((base.id, consumed_size));
                    }
                }
            }
        }

        for (entity_id, consumed_size) in consumed_items {

            // Push a pickup event.
            world.push_event(Event::EntityPickup {
                id: self.data.id,
                target_id: entity_id,
            });

            // Consume the item entity.
            let Some(Entity::Item(base)) = world.get_entity_mut(entity_id) else { panic!() };
            base.kind.stack.size -= consumed_size;
            if base.kind.stack.size == 0 {
                base.dead = true;
            }

        }

        for (index, item) in main_inv.changes() {
            world.push_event(Event::EntityInventoryItem {
                id: self.data.id,
                index,
                item,
            });
        }

        main_inv.clear_changes();

    }

}
