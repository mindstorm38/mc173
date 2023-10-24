//! Data structure for storing a world (overworld or nether) at runtime.

use std::collections::{HashMap, BTreeSet};
use std::collections::hash_map::Entry;
use std::iter::FusedIterator;
use std::cmp::Ordering;
use std::ops::Add;

use glam::{IVec3, Vec2, DVec3};
use indexmap::IndexSet;

use crate::chunk::{Chunk, calc_chunk_pos, calc_chunk_pos_unchecked, calc_entity_chunk_pos, CHUNK_HEIGHT};
use crate::item::ItemStack;
use crate::util::rand::JavaRandom;
use crate::util::bb::BoundingBox;

use crate::entity::Entity;
use crate::block;


/// Data structure for a whole world.
/// 
/// This structure can be used as a data structure to read and modify the world's content,
/// but it can also be ticked in order to run every world's logic (block ticks, random 
/// ticks, entity ticks, time, weather, etc.). When manually modifying or ticking the
/// world, events are produced *(of type [`Event`])* depending on what's modified. **By
/// default**, events are not saved, but an events queue can be swapped in or out of the
/// world to enable or disable events registration.
/// 
/// TODO: Make a diagram to better explain the world structure with entity caching.
pub struct World {
    /// Some events queue if enabled.
    events: Option<Vec<Event>>,
    /// The dimension
    dimension: Dimension,
    /// The spawn position.
    spawn_pos: DVec3,
    /// The world time, increasing at each tick.
    time: u64,
    /// The world's global random number generator.
    rand: JavaRandom,
    /// Mapping of chunks to their coordinates.
    chunks: HashMap<(i32, i32), WorldChunk>,
    /// The entities are stored inside an option, this has no overhead because of niche 
    /// in the box type, but allows us to temporarily own the entity when updating it, 
    /// therefore avoiding borrowing issues.
    entities: Vec<WorldEntity>,
    /// Entities' index mapping from their unique id.
    entities_map: HashMap<u32, usize>,
    /// List of entities that are not belonging to any chunk at the moment.
    orphan_entities: IndexSet<usize>,
    /// Index of the currently updated entity.
    updating_entity_index: Option<usize>,
    /// Next entity id apply to a newly spawned entity.
    next_entity_id: u32,
    /// Mapping of scheduled ticks in the future.
    scheduled_ticks: BTreeSet<ScheduledTick>,
}

impl World {

    /// Create a new world of the given dimension with no events queue by default, so
    /// events are disabled.
    pub fn new(dimension: Dimension) -> Self {
        Self {
            events: None,
            dimension,
            spawn_pos: DVec3::ZERO,
            time: 0,
            rand: JavaRandom::new_seeded(),
            chunks: HashMap::new(),
            entities: Vec::new(),
            entities_map: HashMap::new(),
            orphan_entities: IndexSet::new(),
            updating_entity_index: None,
            next_entity_id: 0,
            scheduled_ticks: BTreeSet::new(),
        }
    }

    /// This function can be used to swap in a new events queue and return the previous
    /// one if relevant. Giving *None* events queue disable events registration using
    /// the [`push_event`] method. Swapping out the events is the only way of reading
    /// them afterward.
    pub fn swap_events(&mut self, events: Option<Vec<Event>>) -> Option<Vec<Event>> {
        std::mem::replace(&mut self.events, events)
    }

    /// Return true if this world has an internal events queue that enables usage of the
    /// [`push_event`] method.
    pub fn has_events(&self) -> bool {
        self.events.is_some()
    }

    /// Push an event in this world. This only actually push the event if events are 
    /// enabled. Events queue can be swapped using [`swap_events`] method.swap_events
    #[inline]
    pub fn push_event(&mut self, event: Event) {
        if let Some(events) = &mut self.events {
            events.push(event);
        }
    }

    pub fn dimension(&self) -> Dimension {
        self.dimension
    }

    /// Get the world's spawn position.
    pub fn spawn_position(&self) -> DVec3 {
        self.spawn_pos
    }

    /// Set the world's spawn position, this triggers `SpawnPosition` event.
    pub fn set_spawn_position(&mut self, pos: DVec3) {
        self.spawn_pos = pos;
        self.push_event(Event::SpawnPosition { pos });
    }

    pub fn time(&self) -> u64 {
        self.time
    }

    pub fn set_time(&mut self, time: u64) {
        self.time = time;
    }

    pub fn rand_mut(&mut self) -> &mut JavaRandom {
        &mut self.rand
    }

    /// Get a reference to a chunk, if existing.
    pub fn chunk(&self, cx: i32, cz: i32) -> Option<&Chunk> {
        self.chunks.get(&(cx, cz)).map(|c| &*c.inner)
    }

    /// Get a mutable reference to a chunk, if existing.
    pub fn chunk_mut(&mut self, cx: i32, cz: i32) -> Option<&mut Chunk> {
        self.chunks.get_mut(&(cx, cz)).map(|c| &mut *c.inner)
    }

    /// Insert a new chunk into the world. This function also generate an event for a new
    /// chunk and also take cares of internal coherency with potential orphan entities 
    /// that are placed in this new chunk.
    pub fn insert_chunk(&mut self, cx: i32, cz: i32, chunk: Box<Chunk>) {
        match self.chunks.entry((cx, cz)) {
            // There was no chunk here, we check in orphan entities if there are entities
            // currently in this chunk's position.
            Entry::Vacant(v) => {

                let mut world_chunk = WorldChunk {
                    inner: chunk,
                    entities: IndexSet::new(),
                };

                self.orphan_entities.retain(|&entity_index| {
                    let entity = &mut self.entities[entity_index];
                    // If the entity is in the newly added chunk.
                    if (entity.cx, entity.cz) == (cx, cz) {
                        world_chunk.entities.insert(entity_index);
                        entity.orphan = false;
                        // Do not retain entity, remove it from orphan list.
                        false
                    } else {
                        true
                    }
                });

                v.insert(world_chunk);

            }
            // The chunk is being replaced, we just transfer all entity to the new one.
            Entry::Occupied(mut o) => {
                // Replace the previous chunk and then move all of its entities to it.
                let prev_chunk = o.insert(WorldChunk {
                    inner: chunk,
                    entities: IndexSet::new(),
                });
                o.get_mut().entities = prev_chunk.entities;
            }
        }
    }

    /// Remove a chunk that may not exists. If the chunk exists, all of its owned entities 
    /// will be transferred to the orphan entities list to be later picked up by another
    /// chunk.
    pub fn remove_chunk(&mut self, cx: i32, cz: i32) -> Option<Box<Chunk>> {
        
        let chunk = self.chunks.remove(&(cx, cz))?;
        for &entity_index in &chunk.entities {
            self.entities[entity_index].orphan = true;
        }

        self.orphan_entities.extend(chunk.entities.into_iter());
        Some(chunk.inner)

    }

    /// Internal function to ensure monomorphization and reduce bloat of the 
    /// generic [`spawn_entity`].
    #[inline(never)]
    fn spawn_entity_inner(&mut self, mut entity: Box<Entity>) -> u32 {

        // Initial position is used to known in which chunk to cache it.
        let entity_base = entity.base_mut();
        let entity_index = self.entities.len();

        // Get the next unique entity id.
        let id = self.next_entity_id;
        self.next_entity_id = self.next_entity_id.checked_add(1)
            .expect("entity id overflow");

        entity_base.id = id;

        // Bind the entity to an existing chunk if possible.
        let (cx, cz) = calc_entity_chunk_pos(entity_base.pos);
        let mut world_entity = WorldEntity {
            inner: Some(entity),
            id,
            cx,
            cz,
            orphan: false,
            bb: None,
        };
        
        if let Some(chunk) = self.chunks.get_mut(&(cx, cz)) {
            chunk.entities.insert(entity_index);
        } else {
            self.orphan_entities.insert(entity_index);
            world_entity.orphan = true;
        }

        self.entities.push(world_entity);
        self.entities_map.insert(id, entity_index);

        self.push_event(Event::EntitySpawn { id });
        id

    }

    /// Spawn an entity in this world, this function gives it a unique id and ensure 
    /// coherency with chunks cache.
    /// 
    /// **This function is legal to call from ticking entities, but such entities will be
    /// ticked once in the same cycle as the currently ticking entity.**
    #[inline(always)]
    pub fn spawn_entity(&mut self, entity: impl Into<Box<Entity>>) -> u32 {
        // NOTE: This method is just a wrapper to ensure boxed entity.
        self.spawn_entity_inner(entity.into())
    }

    /// Kill an entity given its id, this function ensure coherency with chunks cache.
    /// This returns false if the entity is not existing.
    /// 
    /// **This function is legal for entities to call on themselves when ticking.**
    pub fn kill_entity(&mut self, id: u32) -> bool {

        let Some(entity_index) = self.entities_map.remove(&id) else { return false };
        let killed_entity = self.entities.swap_remove(entity_index);

        // Remove the killed entity from the chunk it belongs to.
        if killed_entity.orphan {
            &mut self.orphan_entities
        } else {
            &mut self.chunks.get_mut(&(killed_entity.cx, killed_entity.cz))
                .expect("non-orphan entity referencing a non-existing chunk")
                .entities
        }.remove(&entity_index);

        // If we are removing the entity being updated, set its index to none so it will
        // not placed back into its slot.
        if self.updating_entity_index == Some(entity_index) {
            self.updating_entity_index = None;
        }

        // Because we used swap remove, this may have moved the last entity (if
        // existing) to the old entity index. We need to update its index in chunk
        // or orphan entities.
        if let Some(entity) = self.entities.get(entity_index) {

            // Get the correct entities list depending on the entity being orphan or not.
            let entities = if entity.orphan {
                &mut self.orphan_entities
            } else {
                &mut self.chunks.get_mut(&(entity.cx, entity.cz))
                    .expect("non-orphan entity referencing a non-existing chunk")
                    .entities
            };

            // The swapped entity was at the end, so the new length.
            let previous_index = self.entities.len();

            // Update the mapping from entity unique id to the new index.
            let previous_map_index = self.entities_map.insert(entity.id, entity_index);
            debug_assert_eq!(previous_map_index, Some(previous_index), "incoherent previous entity index");

            // The entity that was swapped is the entity being updated, we need to change
            // its index so it will be placed back into the correct slot.
            if self.updating_entity_index == Some(previous_index) {
                self.updating_entity_index = Some(entity_index);
            }

            let remove_success = entities.remove(&previous_index);
            debug_assert!(remove_success, "entity index not found where it belongs");
            entities.insert(entity_index);

        }

        self.push_event(Event::EntityKill { id });
        true

    }

    /// Get a generic entity from its unique id. This generic entity can later be checked
    /// for being of a particular type.
    #[track_caller]
    pub fn entity(&self, id: u32) -> Option<&Entity> {
        let index = *self.entities_map.get(&id)?;
        Some(self.entities[index].inner
            .as_deref()
            .expect("entity is being updated"))
    }

    /// Get a generic entity from its unique id. This generic entity can later be checked
    /// for being of a particular type.
    #[track_caller]
    pub fn entity_mut(&mut self, id: u32) -> Option<&mut Entity> {
        let index = *self.entities_map.get(&id)?;
        Some(self.entities[index].inner
            .as_deref_mut()
            .expect("entity is being updated"))
    }

    /// Schedule a tick update to happen at the given position, for the given block id
    /// and in a given time.
    pub fn schedule_tick(&mut self, pos: IVec3, id: u8, time: u64) {
        self.scheduled_ticks.insert(ScheduledTick {
            time: self.time + time,
            pos,
            id,
        });
    }

    /// Iterate over all blocks in the given area.
    /// *Min is inclusive and max is exclusive.*
    pub fn iter_blocks_in(&self, min: IVec3, max: IVec3) -> impl Iterator<Item = (IVec3, u8, u8)> + '_ {
        WorldBlocksIn::new(self, min, max)
    }

    /// Iterate over all blocks that are in the bounding box area, this doesn't check for
    /// actual collision with the block's bounding box, it just return all potential 
    /// blocks in the bounding box' area.
    pub fn iter_blocks_in_box(&self, bb: BoundingBox) -> impl Iterator<Item = (IVec3, u8, u8)> + '_ {
        let min = bb.min.floor().as_ivec3();
        let max = bb.max.add(1.0).floor().as_ivec3();
        self.iter_blocks_in(min, max)
    }

    /// Iterate over all bounding boxes in the given area.
    /// *Min is inclusive and max is exclusive.*
    pub fn iter_blocks_boxes_in(&self, min: IVec3, max: IVec3) -> impl Iterator<Item = BoundingBox> + '_ {
        self.iter_blocks_in(min, max).flat_map(|(pos, block, metadata)| {
            let pos = pos.as_dvec3();
            block::from_id(block).bounding_boxes(block, metadata).iter()
                .map(move |bb| bb.offset(pos))
        })
    }

    /// Iterate over all bounding boxes in the given area that are colliding with the 
    /// given one.
    pub fn iter_blocks_boxes_colliding(&self, bb: BoundingBox) -> impl Iterator<Item = BoundingBox> + '_ {
        let min = bb.min.floor().as_ivec3();
        let max = bb.max.add(1.0).floor().as_ivec3();
        self.iter_blocks_boxes_in(min, max)
            .filter(move |block_bb| block_bb.intersects(bb))
    }

    /// Iterate over all entities in the world.
    pub fn iter_entities(&self) -> impl Iterator<Item = &Entity> {
        self.entities.iter()
            .filter_map(|e| e.inner.as_ref())
            .map(|e| &**e)
    }

    /// Internal function to iterate world entities in a given chunk.
    fn iter_world_entities_in(&self, cx: i32, cz: i32) -> impl Iterator<Item = &WorldEntity> {

        let (entities, orphan) = self.chunks.get(&(cx, cz))
            .map(|c| (&c.entities, false))
            .unwrap_or((&self.orphan_entities, true));

        entities.iter()
            .filter_map(move |&entity_index| {
                let world_entity = &self.entities[entity_index];
                debug_assert_eq!(world_entity.orphan, orphan, "incoherent orphan entity");
                // If we are iterating the orphan entities, check the chunk.
                if orphan {
                    if (world_entity.cx, world_entity.cz) != (cx, cz) {
                        return None;
                    }
                }
                Some(world_entity)
            })
        
    }

    /// Iterate over all entities of the given chunk. This is legal for non-existing 
    /// chunks, in such case this will search for orphan entities.
    /// *This function can't return the current updated entity.*
    pub fn iter_entities_in(&self, cx: i32, cz: i32) -> impl Iterator<Item = &Entity> {
        self.iter_world_entities_in(cx, cz).filter_map(|world_entity| {
            world_entity.inner.as_deref()
        })
    }

    /// Iterate over all entities colliding with the given bounding box.
    /// *This function can't return the current updated entity.*
    pub fn iter_entities_boxes_colliding(&self, bb: BoundingBox) -> impl Iterator<Item = (&Entity, BoundingBox)> {

        let (min_cx, min_cz) = calc_entity_chunk_pos(bb.min - 2.0);
        let (max_cx, max_cz) = calc_entity_chunk_pos(bb.max + 2.0);

        (min_cx..=max_cx).flat_map(move |cx| (min_cz..=max_cz).map(move |cz| (cx, cz)))
            .flat_map(move |(cx, cz)| {
                self.iter_world_entities_in(cx, cz).filter_map(move |entity| {
                    if let (Some(entity), Some(entity_bb)) = (&entity.inner, entity.bb) {
                        bb.intersects(entity_bb).then_some((&**entity, entity_bb))
                    } else {
                        None
                    }
                })
            })

    }

    /// Iterate over all bounding box in the world that collides with the given one, this
    /// includes blocks and entities bounding boxes. *Note however that this will not 
    /// return the bounding box of the updating entity.*
    pub fn iter_boxes_colliding(&self, bb: BoundingBox) -> impl Iterator<Item = BoundingBox> + '_ {
        let bb_for_entities = bb.inflate(DVec3::splat(0.25));
        self.iter_blocks_boxes_colliding(bb)
            .chain(self.iter_entities_boxes_colliding(bb_for_entities).map(|(_, bb)| bb))
    }

    /// Get block and metadata at given position in the world, if the chunk is not
    /// loaded, none is returned.
    pub fn block_and_metadata(&self, pos: IVec3) -> Option<(u8, u8)> {
        let (cx, cz) = calc_chunk_pos(pos)?;
        let chunk = self.chunk(cx, cz)?;
        Some(chunk.block_and_metadata(pos))
    }

    /// Set block and metadata at given position in the world, if the chunk is not
    /// loaded, none is returned, but if it is existing the previous block and metadata
    /// is returned. This function also push a block change event.
    pub fn set_block_and_metadata(&mut self, pos: IVec3, block: u8, metadata: u8) -> Option<(u8, u8)> {
        let (cx, cz) = calc_chunk_pos(pos)?;
        let chunk = self.chunk_mut(cx, cz)?;
        let (prev_block, prev_metadata) = chunk.block_and_metadata(pos);
        chunk.set_block_and_metadata(pos, block, metadata);
        self.push_event(Event::BlockChange { 
            pos,
            prev_block, 
            prev_metadata, 
            new_block: block, 
            new_metadata: metadata,
        });
        Some((prev_block, prev_metadata))
    }

    /// Break a block naturally and drop its items. This function will generate an event 
    /// of the block break and the items spawn. This returns true if successful, false
    /// if the chunk/pos was not valid.
    pub fn break_block(&mut self, pos: IVec3) -> bool {
        if let Some((prev_id, prev_metadata)) = self.set_block_and_metadata(pos, 0, 0) {
            block::drop::drop_at(self, pos, prev_id, prev_metadata, 1.0);
            true
        } else {
            false
        }
    }

    /// Tick the world, this ticks all entities.
    pub fn tick(&mut self) {

        self.time += 1;

        // Schedule ticks...
        while let Some(tick) = self.scheduled_ticks.first() {
            if self.time >= tick.time {
                // This tick should be activated.
                let tick = self.scheduled_ticks.pop_first().unwrap();
                // Check coherency of the scheduled tick and current block.
                if let Some((id, metadata)) = self.block_and_metadata(tick.pos) {
                    if id == tick.id {
                        block::tick::tick_at(self, tick.pos, id, metadata);
                    }
                }
            } else {
                // Our set is ordered by time first, so we break when past current time. 
                break;
            }
        }

        // Update every entity's bounding box prior to actually ticking.
        for world_entity in &mut self.entities {
            // NOTE: Unwrapping because entities should not be updating here.
            let entity = world_entity.inner.as_ref().unwrap();
            let entity_base = entity.base();
            world_entity.bb = entity_base.coherent.then_some(entity_base.bb);
        }

        // NOTE: We don't use a for loop because killed and spawned entities may change
        // the length of the list.
        let mut i = 0;
        while i < self.entities.len() {

            // We keep a reference to the currently updated entity, this allows us to 
            // react to the following events:
            // - The updating entity is killed, and another entity is swapped at its 
            //   index: we don't want to increment the index to tick swapped entity,
            //   we also don't want to reinsert the entity.
            // - Another entity was killed, but the updating entity was the last one and
            //   it swapped with the removed one, its index changed.
            self.updating_entity_index = Some(i);

            // We unwrap because all entities should be present except updated one.
            let mut entity = self.entities[i].inner.take().unwrap();
            entity.tick(&mut *self);

            // Check if the entity is still alive, if not we just continue without
            // incrementing 'i' to tick the entity that was moved in place.
            let Some(entity_index) = self.updating_entity_index else { continue };

            // Before re-adding the entity, check dirty flags to send proper events.
            let entity_base = entity.base_mut();
            let mut new_chunk = None;

            // If the position is dirty, compute the expected chunk position and trigger
            // an entity position event.
            if std::mem::take(&mut entity_base.pos_dirty) {
                new_chunk = Some(calc_entity_chunk_pos(entity_base.pos));
                self.push_event(Event::EntityPosition { id: entity_base.id, pos: entity_base.pos });
            }

            // If the look is dirt, trigger an entity look event.
            if std::mem::take(&mut entity_base.look_dirty) {
                self.push_event(Event::EntityLook { id: entity_base.id, look: entity_base.look });
            }

            // After tick, we re-add the entity.
            let world_entity = &mut self.entities[i];
            debug_assert!(world_entity.inner.is_none(), "incoherent updating entity");
            world_entity.inner = Some(entity);

            // Check the potential new chunk position.
            if let Some((new_cx, new_cz)) = new_chunk {
                // Check if the entity chunk position has changed.
                if (world_entity.cx, world_entity.cz) != (new_cx, new_cz) {

                    // Get the previous entities list, where the current entity should
                    // be cached in order to remove it.
                    let entities = if world_entity.orphan {
                        &mut self.orphan_entities
                    } else {
                        &mut self.chunks.get_mut(&(world_entity.cx, world_entity.cz))
                            .expect("non-orphan entity referencing a non-existing chunk")
                            .entities
                    };

                    let remove_success = entities.remove(&entity_index);
                    debug_assert!(remove_success, "entity index not found where it belongs");

                    // Update the world entity to its new chunk and orphan state.
                    world_entity.cx = new_cx;
                    world_entity.cz = new_cz;
                    if let Some(chunk) = self.chunks.get_mut(&(new_cx, new_cz)) {
                        world_entity.orphan = false;
                        chunk.entities.insert(entity_index);
                    } else {
                        world_entity.orphan = true;
                        self.orphan_entities.insert(entity_index);
                    }

                }
            }

            // Only increment if the entity index has not changed, changed index means
            // that our entity has been swapped to a previous index, because it was the
            // last one. But new new entity may have been spawned, so a new entity may
            // have replaced at 'i'.
            if entity_index == i {
                i += 1;
            }

        }

    }

}


/// Types of dimensions, used for ambient effects in the world.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Dimension {
    /// The overworld dimension with a blue sky and day cycles.
    Overworld,
    /// The creepy nether dimension.
    Nether,
}

/// An event that happened in the world.
#[derive(Debug, Clone, Copy)]
pub enum Event {
    /// A new entity has been spawned.
    EntitySpawn {
        /// The unique id of the spawned entity.
        id: u32,
    },
    /// An entity has been killed from the world.
    EntityKill {
        /// The unique id of the killed entity.
        id: u32,
    },
    /// An entity has moved.
    EntityPosition {
        /// The unique id of the entity.
        id: u32,
        /// Absolute position of the entity.
        pos: DVec3,
    },
    /// An entity changed its look angles.
    EntityLook {
        /// The unique id of the entity.
        id: u32,
        /// The entity look.
        look: Vec2,
    },
    /// An entity has collected another entity on ground, this is usually an item or 
    /// arrow entity picked up by a player entity.
    EntityPickup {
        /// The entity that collected an item.
        id: u32,
        /// The target entity that was collected.
        target_id: u32,
    },
    /// An entity had an item change in its inventory. This is usually a player getting
    /// new items in its inventory.
    EntityInventoryItem {
        /// Entity id.
        id: u32,
        /// Index of the slot where the item changed.
        index: usize,
        /// The item stack at the given index.
        item: ItemStack,
    },
    /// A block has been changed in the world.
    BlockChange {
        /// The block position.
        pos: IVec3,
        /// Previous block id.
        prev_block: u8,
        /// Previous block metadata.
        prev_metadata: u8,
        /// The new block id.
        new_block: u8,
        /// The new block metadata.
        new_metadata: u8,
    },
    /// The world's spawn point has been changed.
    SpawnPosition {
        /// The new spawn point position.
        pos: DVec3,
    }
}


/// Internal type for storing a world chunk with its cached entities.
struct WorldChunk {
    /// Underlying chunk.
    inner: Box<Chunk>,
    /// Entities belonging to this chunk.
    entities: IndexSet<usize>,
}

/// Internal type for storing a world entity and keep track of its current chunk.
struct WorldEntity {
    /// Underlying entity, the none variant is rare and only happen once per tick when
    /// the chunk is updated.
    inner: Option<Box<Entity>>,
    /// Unique entity id is duplicated here to allow us to access it event when entity
    /// is updating.
    id: u32,
    /// The last computed chunk position X.
    cx: i32,
    /// The last computed chunk position Z.
    cz: i32,
    /// Indicate if this entity is orphan and therefore does not belong to any chunk.
    orphan: bool,
    /// The bounding box of this entity prior to ticking, none is used if the entity
    /// bounding box isn't coherent, which is the default when the entity has just been
    /// spawned.
    bb: Option<BoundingBox>,
}

/// A block tick scheduled in the future, it's associated to a world time in a tree map.
/// This structure is ordered by time and then by position, this allows to have multiple
/// block update at the same time but for different positions.
#[derive(PartialEq, Eq)]
struct ScheduledTick {
    /// The time to tick the block.
    time: u64,
    /// Position of the block to tick.
    pos: IVec3,
    /// The expected id of the block, if the block has no longer this id, this tick is
    /// ignored.
    id: u8,
}

impl PartialOrd for ScheduledTick {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(Ord::cmp(self, other))
    }
}

impl Ord for ScheduledTick {
    fn cmp(&self, other: &Self) -> Ordering {
        self.time.cmp(&other.time)
            .then(self.pos.x.cmp(&other.pos.x))
            .then(self.pos.z.cmp(&other.pos.z))
            .then(self.pos.y.cmp(&other.pos.y))
    }
}


/// An iterator for blocks in a world area. This returns the block id and metadata.
struct WorldBlocksIn<'a> {
    /// Back-reference to the containing world.
    world: &'a World,
    /// This contains a temporary reference to the chunk being analyzed. This is used to
    /// avoid repeatedly fetching chunks' map.
    chunk: Option<(i32, i32, Option<&'a Chunk>)>,
    /// Minimum coordinate to fetch.
    min: IVec3,
    /// Maximum coordinate to fetch (exclusive).
    max: IVec3,
    /// Next block to fetch.
    cursor: IVec3,
}

impl<'a> WorldBlocksIn<'a> {

    #[inline]
    fn new(world: &'a World, mut min: IVec3, mut max: IVec3) -> Self {

        debug_assert!(min.x <= max.x && min.y <= max.y && min.z <= max.z);

        min.y = min.y.clamp(0, CHUNK_HEIGHT as i32 - 1);
        max.y = max.y.clamp(0, CHUNK_HEIGHT as i32 - 1);

        // If one the component is in common, because max is exclusive, there will be no
        // blocks at all to read, so we set max to min so it will directly ends.
        if min.x == max.x || min.y == max.y || min.z == max.z {
            max = min;
        }

        WorldBlocksIn {
            world,
            chunk: None,
            min,
            max,
            cursor: min,
        }

    }

}

impl<'a> FusedIterator for WorldBlocksIn<'a> {}
impl<'a> Iterator for WorldBlocksIn<'a> {

    type Item = (IVec3, u8, u8);

    fn next(&mut self) -> Option<Self::Item> {
        
        let cursor = self.cursor;

        // X is the last updated component, so when it reaches max it's done.
        if cursor.x == self.max.x {
            return None;
        }

        // We are at the start of a new column, update the chunk.
        if cursor.y == self.min.y {
            // NOTE: Unchecked because the Y value is clamped in the constructor.
            let (cx, cz) = calc_chunk_pos_unchecked(cursor);
            if !matches!(self.chunk, Some((ccx, ccz, _)) if (ccx, ccz) == (cx, cz)) {
                self.chunk = Some((cx, cz, self.world.chunk(cx, cz)));
            }
        }

        // If there is no chunk at the position, defaults to (id = 0, metadata = 0).
        let mut ret = (self.cursor, 0, 0);

        // If a chunk exists for the current column.
        if let Some((_, _, Some(chunk))) = self.chunk {
            let (block, metadata) = chunk.block_and_metadata(self.cursor);
            ret.1 = block;
            ret.2 = metadata;
        }

        self.cursor.y += 1;
        if self.cursor.y == self.max.y {
            self.cursor.y = self.min.y;
            self.cursor.z += 1;
            if self.cursor.z == self.max.z {
                self.cursor.z = self.min.z;
                self.cursor.x += 1;
            }
        }

        Some(ret)

    }

}
