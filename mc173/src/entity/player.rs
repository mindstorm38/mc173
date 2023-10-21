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

        let mut consumed_items = Vec::new();

        for (entity, _) in world.iter_entities_boxes_colliding(self.bb.inflate(DVec3::new(1.0, 0.0, 1.0))) {
            if let Entity::Item(base) = entity {
                if base.kind.frozen_ticks == 0 {
                    // Add the pickup item to the main inventory.
                    let picked_item = base.kind.item;
                    let consumed_size = self.kind.kind.inventory.main.add_item(picked_item);
                    if consumed_size != 0 {
                        consumed_items.push((base.id, consumed_size));
                    }
                }
            }
        }

        for (entity_id, consumed_size) in consumed_items {
            // Push a pickup event.
            world.push_event(Event::EntityPickup {
                id: self.id,
                target_id: entity_id,
            });
            // Consume the item entity.
            let Some(Entity::Item(base)) = world.entity_mut(entity_id) else { panic!() };
            base.kind.item.size -= consumed_size;
            if base.kind.item.size == 0 {
                world.kill_entity(entity_id);
            }
        }

    }

}
