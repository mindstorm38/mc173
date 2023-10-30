//! Data structure for storing a world (overworld or nether) at runtime.

use std::collections::{HashMap, BTreeSet, HashSet};
use std::collections::hash_map::Entry;
use std::iter::FusedIterator;
use std::cmp::Ordering;
use std::ops::Add;

use glam::{IVec3, Vec2, DVec3};
use indexmap::IndexSet;

use crate::chunk::{Chunk, calc_chunk_pos, calc_chunk_pos_unchecked, calc_entity_chunk_pos, CHUNK_HEIGHT, CHUNK_WIDTH};
use crate::util::{JavaRandom, BoundingBox, Face};
use crate::item::ItemStack;

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
    /// When enabled, this contains the list of events that happened in the world since
    /// it was last swapped. This swap behavior is really useful in order to avoid 
    /// borrowing issues, by temporarily taking ownership of events, the caller can get
    /// a mutable reference to that world at the same time.
    events: Option<Vec<Event>>,
    /// The dimension
    dimension: Dimension,
    /// The spawn position.
    spawn_pos: DVec3,
    /// The world time, increasing on each tick. This is used for day/night cycle but 
    /// also for registering scheduled ticks.
    time: u64,
    /// The world's global random number generator, it is used everywhere to randomize
    /// events in the world, such as plant grow.
    rand: JavaRandom,
    /// Mapping of chunks to their coordinates. Each chunk is a wrapper type because the
    /// raw chunk structure do not care of entities, this wrapper however keep track for
    /// each chunk of the entities in it.
    chunks: HashMap<(i32, i32), WorldChunk>,
    /// Next entity id apply to a newly spawned entity.
    entities_next_id: u32,
    /// The entities are stored inside an option, this has no overhead because of niche 
    /// in the box type, but allows us to temporarily own the entity when updating it, 
    /// therefore avoiding borrowing issues.
    entities: Vec<WorldEntity>,
    /// Entities' index mapping from their unique id.
    entities_map: HashMap<u32, usize>,
    /// Queue of dead entities that should be removed after being updated. We keep this
    /// queue in the world structure to avoid frequent allocation. This queue contains
    /// entity indices and should be in ascending order.
    entities_dead: Vec<usize>,
    /// Set of entities that are not belonging to any chunk at the moment.
    entities_orphan: IndexSet<usize>,
    /// Mapping of scheduled ticks in the future.
    scheduled_ticks: BTreeSet<ScheduledTick>,
    /// A set of all scheduled tick states, used to avoid ticking twice the same position
    /// and block id. 
    scheduled_ticks_states: HashSet<ScheduledTickState>,
    /// This is the wrapping seed used by random ticks to compute random block positions.
    random_ticks_seed: i32,
    /// Internal cached queue of random ticks that should be computed, this queue is only
    /// used in the random ticking engine. It is put in an option in order to be owned
    /// while random ticking and therefore avoiding borrowing issue with the world.
    pending_random_ticks: Option<Vec<(IVec3, u8, u8)>>,
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
            entities_next_id: 0,
            entities: Vec::new(),
            entities_map: HashMap::new(),
            entities_dead: Vec::new(),
            entities_orphan: IndexSet::new(),
            scheduled_ticks: BTreeSet::new(),
            scheduled_ticks_states: HashSet::new(),
            random_ticks_seed: JavaRandom::new_seeded().next_int(),
            pending_random_ticks: Some(Vec::new()),
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

                self.entities_orphan.retain(|&entity_index| {
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

        self.entities_orphan.extend(chunk.entities.into_iter());
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
        let id = self.entities_next_id;
        self.entities_next_id = self.entities_next_id.checked_add(1)
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
            self.entities_orphan.insert(entity_index);
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
        let state = ScheduledTickState { pos, id };
        if self.scheduled_ticks_states.insert(state) {
            self.scheduled_ticks.insert(ScheduledTick { time: self.time + time, state });
        }
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
        self.iter_blocks_in(min, max).flat_map(|(pos, id, metadata)| {
            block::colliding::iter_colliding_box(self, pos, id, metadata)
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
            .unwrap_or((&self.entities_orphan, true));

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
    pub fn iter_entities_colliding(&self, bb: BoundingBox) -> impl Iterator<Item = (&Entity, BoundingBox)> {

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

    /// Ray trace from an origin point and return the first colliding blocks, either 
    /// entity or block. Caller can choose to hit fluid blocks or not.
    pub fn ray_trace_blocks(&self, origin: DVec3, ray: DVec3, fluid: bool) -> Option<(IVec3, Face)> {
        
        let ray_norm = ray.normalize();

        let mut pos = origin;
        let mut block_pos = pos.floor().as_ivec3();
        let stop_pos = origin.add(ray).floor().as_ivec3();

        // Break when an invalid chunk is encountered.
        while let Some((id, metadata)) = self.block(block_pos) {

            let mut should_check = true;
            if fluid && matches!(id, block::WATER_MOVING | block::WATER_STILL | block::LAVA_MOVING | block::LAVA_STILL) {
                should_check = block::fluid::is_source(metadata);
            }

            if should_check {
                if let Some(bb) = block::colliding::get_overlay_box(self, block_pos, id, metadata) {
                    if let Some((_, face)) = bb.calc_ray_trace(origin, ray) {
                        return Some((block_pos, face));
                    }
                }
            }

            // Reached the last block position, just break!
            if block_pos == stop_pos {
                break;
            }

            // Binary search algorithm of the next adjacent block to check.
            let mut tmp_norm = ray_norm;
            let mut next_block_pos;

            'a: loop {

                pos += tmp_norm;
                next_block_pos = pos.floor().as_ivec3();

                // If we reached another block, tmp norm is divided by two in order to
                // converge toward the nearest block.
                // FIXME: Maybe put a limit in the norm value, to avoid searching 
                // for infinitesimal collisions.
                if next_block_pos != block_pos {
                    tmp_norm /= 2.0;
                }

                // The next pos is different, check if it is on a face, or 
                while next_block_pos != block_pos {

                    // We check the delta between current block pos and the next one, we 
                    // check if this new pos is on a face of the current pos.
                    let pos_delta = (next_block_pos - block_pos).abs();

                    // Manhattan distance == 1 means we are on a face, use this pos for 
                    // the next ray trace test.
                    if pos_delta.x + pos_delta.y + pos_delta.z == 1 {
                        break 'a;
                    }

                    // Go backward and try finding a block nearer our current pos.
                    pos -= tmp_norm;
                    next_block_pos = pos.floor().as_ivec3();

                }

            }

            block_pos = next_block_pos;

        }

        None

    }

    /// Get block and metadata at given position in the world, if the chunk is not
    /// loaded, none is returned.
    /// 
    /// TODO: Work on a world's block cache to speed up access.
    pub fn block(&self, pos: IVec3) -> Option<(u8, u8)> {
        let (cx, cz) = calc_chunk_pos(pos)?;
        let chunk = self.chunk(cx, cz)?;
        Some(chunk.block(pos))
    }

    /// Set block and metadata at given position in the world, if the chunk is not
    /// loaded, none is returned, but if it is existing the previous block and metadata
    /// is returned. This function also push a block change event.
    pub fn set_block(&mut self, pos: IVec3, id: u8, metadata: u8) -> Option<(u8, u8)> {
        let (cx, cz) = calc_chunk_pos(pos)?;
        let chunk = self.chunk_mut(cx, cz)?;
        let (prev_id, prev_metadata) = chunk.block(pos);
        if prev_id != id || prev_metadata != metadata {
            chunk.set_block(pos, id, metadata);
            self.push_event(Event::BlockChange {
                pos,
                prev_id, 
                prev_metadata, 
                new_id: id,
                new_metadata: metadata,
            });
        }
        Some((prev_id, prev_metadata))
    }

    /// Same as the `set_block` method, but the previous block and new block are notified
    /// of that removal and addition.
    pub fn set_block_self_notify(&mut self, pos: IVec3, id: u8, metadata: u8) -> Option<(u8, u8)> {
        let (prev_id, prev_metadata) = self.set_block(pos, id, metadata)?;
        block::notifying::self_notify_at(self, pos, prev_id, prev_metadata, id, metadata);
        Some((prev_id, prev_metadata))
    }

    /// Same as the `set_block_self_notify` method, but additionally the blocks around 
    /// are notified of that neighbor change. 
    pub fn set_block_notify(&mut self, pos: IVec3, id: u8, metadata: u8) -> Option<(u8, u8)> {
        let (prev_id, prev_metadata) = self.set_block_self_notify(pos, id, metadata)?;
        block::notifying::notify_around(self, pos);
        Some((prev_id, prev_metadata))
    }
    
    /// Tick the world, this ticks all entities.
    pub fn tick(&mut self) {
        self.time += 1;
        self.tick_scheduler();
        self.tick_randomly();
        self.tick_entities();
        // TODO: tick block entities.
    }

    /// Internal function to tick the internal scheduler.
    fn tick_scheduler(&mut self) {

        debug_assert_eq!(self.scheduled_ticks.len(), self.scheduled_ticks_states.len());

        // Schedule ticks...
        while let Some(tick) = self.scheduled_ticks.first() {
            if self.time >= tick.time {
                // This tick should be activated.
                let tick = self.scheduled_ticks.pop_first().unwrap();
                assert!(self.scheduled_ticks_states.remove(&tick.state));
                // Check coherency of the scheduled tick and current block.
                if let Some((id, metadata)) = self.block(tick.state.pos) {
                    if id == tick.state.id {
                        block::ticking::tick_at(self, tick.state.pos, id, metadata);
                    }
                }
            } else {
                // Our set is ordered by time first, so we break when past current time. 
                break;
            }
        }

    }

    /// Internal function to randomly tick loaded chunks. This also include random weather
    /// events such as snow block being placed and lightning strikes.
    fn tick_randomly(&mut self) {

        let mut pending_random_ticks = self.pending_random_ticks.take().unwrap();
        debug_assert!(pending_random_ticks.is_empty());

        for (&(cx, cz), chunk) in &mut self.chunks {

            // TODO: Lightning strikes.
            // TODO: Random snowing.

            let chunk_pos = IVec3::new(cx * CHUNK_WIDTH as i32, 0, cz * CHUNK_WIDTH as i32);
            
            // Minecraft run 80 random ticks per tick per chunk.
            for _ in 0..80 {

                self.random_ticks_seed = self.random_ticks_seed
                    .wrapping_mul(3)
                    .wrapping_add(1013904223);

                let rand = self.random_ticks_seed >> 2;
                let pos = IVec3::new((rand >> 0) & 15, (rand >> 16) & 127, (rand >> 8) & 15);

                let (id, metadata) = chunk.inner.block(pos);
                pending_random_ticks.push((chunk_pos + pos, id, metadata));

            }

        }

        for (pos, id, metadata) in pending_random_ticks.drain(..) {
            block::ticking::random_tick_at(self, pos, id, metadata)
        }

        self.pending_random_ticks = Some(pending_random_ticks);

    }

    /// Internal function to tick all entities.
    fn tick_entities(&mut self) {

        debug_assert_eq!(self.entities.len(), self.entities_map.len());
        debug_assert!(self.entities_dead.is_empty());

        // Update every entity's bounding box prior to actually ticking.
        for world_entity in &mut self.entities {
            // NOTE: Unwrapping because entities should not be updating here.
            let entity = world_entity.inner.as_ref().unwrap();
            let entity_base = entity.base();
            world_entity.bb = entity_base.coherent.then_some(entity_base.bb);
        }

        // NOTE: We don't tick entities added while iterating.
        for i in 0..self.entities.len() {

            // We unwrap because all entities should be present except updated one.
            let mut entity = self.entities[i].inner.take().unwrap();
            entity.tick(&mut *self);

            // Before re-adding the entity, check dirty flags to send proper events.
            let entity_base = entity.base_mut();
            let mut new_chunk = None;

            if entity_base.dead {
                // Add the entity to the despawning queue.
                self.entities_dead.push(i);
                self.push_event(Event::EntityDead { id: entity_base.id });
            } else {

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

                // If the look is dirt, trigger an entity look event.
                if std::mem::take(&mut entity_base.vel_dirty) {
                    self.push_event(Event::EntityVelocity { id: entity_base.id, vel: entity_base.vel });
                }

            }

            // After tick, we re-add the entity.
            let world_entity = &mut self.entities[i];
            debug_assert!(world_entity.inner.is_none(), "incoherent updating entity");
            world_entity.inner = Some(entity);

            // Check if the entity moved to another chunk...
            if let Some((new_cx, new_cz)) = new_chunk {
                if (world_entity.cx, world_entity.cz) != (new_cx, new_cz) {

                    // Get the previous entities list, where the current entity should
                    // be cached in order to remove it.
                    let entities = if world_entity.orphan {
                        &mut self.entities_orphan
                    } else {
                        &mut self.chunks.get_mut(&(world_entity.cx, world_entity.cz))
                            .expect("non-orphan entity referencing a non-existing chunk")
                            .entities
                    };

                    let remove_success = entities.remove(&i);
                    debug_assert!(remove_success, "entity index not found where it belongs");

                    // Update the world entity to its new chunk and orphan state.
                    world_entity.cx = new_cx;
                    world_entity.cz = new_cz;
                    if let Some(chunk) = self.chunks.get_mut(&(new_cx, new_cz)) {
                        world_entity.orphan = false;
                        chunk.entities.insert(i);
                    } else {
                        world_entity.orphan = true;
                        self.entities_orphan.insert(i);
                    }

                }
            }

        }

        // We swap remove each dead entity. We know that this dead queue is sorted in
        // ascending order because we updated index in order. By reversing the iterator
        // we ensure that we'll not modify any dead entity's index, that would by 
        // impossible to manage.
        for entity_index in self.entities_dead.drain(..).rev() {

            let removed_entity = self.entities.swap_remove(entity_index);
            debug_assert!(removed_entity.inner.is_some(), "dead entity is updating");

            let remove_success = self.entities_map.remove(&removed_entity.id).is_some();
            debug_assert!(remove_success, "dead entity was not in entity map");

            // Remove the entity from the chunk it belongs to.
            if removed_entity.orphan {
                &mut self.entities_orphan
            } else {
                &mut self.chunks.get_mut(&(removed_entity.cx, removed_entity.cz))
                    .expect("non-orphan entity referencing a non-existing chunk")
                    .entities
            }.remove(&entity_index);

            // Because we used swap remove, this may have moved the last entity (if
            // existing) to the removed entity index. We need to update its index in 
            // chunk or orphan entities.
            if let Some(entity) = self.entities.get(entity_index) {

                // Get the correct entities list depending on the entity being orphan or not.
                let entities = if entity.orphan {
                    &mut self.entities_orphan
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

                let remove_success = entities.remove(&previous_index);
                debug_assert!(remove_success, "entity index not found where it belongs");
                entities.insert(entity_index);
                
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
    EntityDead {
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
    /// An entity changed its velocity.
    EntityVelocity {
        /// The unique id of the entity.
        id: u32,
        /// The entity velocity.
        vel: DVec3
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
        prev_id: u8,
        /// Previous block metadata.
        prev_metadata: u8,
        /// The new block id.
        new_id: u8,
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

/// A block tick position, this is always linked to a [`ScheduledTick`] being added to
/// the tree map, this structure is also stored appart in order to check that two ticks
/// are not scheduled for the same position and block id.
#[derive(Clone, Copy, Hash, PartialEq, Eq)]
struct ScheduledTickState {
    /// Position of the block to tick.
    pos: IVec3,
    /// The expected id of the block, if the block has no longer this id, this tick is
    /// ignored.
    id: u8,
}

/// A block tick scheduled in the future, it's associated to a world time in a tree map.
/// This structure is ordered by time and then by position, this allows to have multiple
/// block update at the same time but for different positions.
#[derive(PartialEq, Eq)]
struct ScheduledTick {
    /// The time to tick the block.
    time: u64,
    /// State of that scheduled tick.
    state: ScheduledTickState,
}

impl PartialOrd for ScheduledTick {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(Ord::cmp(self, other))
    }
}

impl Ord for ScheduledTick {
    fn cmp(&self, other: &Self) -> Ordering {
        self.time.cmp(&other.time)
            .then(self.state.pos.x.cmp(&other.state.pos.x))
            .then(self.state.pos.z.cmp(&other.state.pos.z))
            .then(self.state.pos.y.cmp(&other.state.pos.y))
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
            let (block, metadata) = chunk.block(self.cursor);
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
