//! Data structure for storing a world (overworld or nether) at runtime.

use std::collections::{HashMap, BTreeSet, HashSet, VecDeque};
use std::collections::hash_map::Entry;
use std::iter::FusedIterator;
use std::cmp::Ordering;

use glam::{IVec3, Vec2, DVec3};
use indexmap::IndexSet;

use crate::block;
use crate::block_entity::BlockEntity;
use crate::chunk::{Chunk, calc_chunk_pos, calc_chunk_pos_unchecked, calc_entity_chunk_pos, CHUNK_HEIGHT, CHUNK_WIDTH};
use crate::util::{JavaRandom, BoundingBox, Face};
use crate::item::ItemStack;

use crate::entity::Entity;


// Following modules are order by order of importance, last modules depends on first ones.
pub mod material;
pub mod bound;
pub mod power;
pub mod loot;
pub mod interact;
pub mod place;
pub mod r#break;
pub mod tick;
pub mod notify;
pub mod light;


/// Data structure for a whole world.
/// 
/// This structure can be used as a data structure to read and modify the world's content,
/// but it can also be ticked in order to run every world's logic (block ticks, random 
/// ticks, entity ticks, time, weather, etc.). When manually modifying or ticking the
/// world, events are produced *(of type [`Event`])* depending on what's modified. **By
/// default**, events are not saved, but an events queue can be swapped in or out of the
/// world to enable or disable events registration.
/// 
/// Methods provided on this structure should follow a naming convention depending on the
/// action that will apply to the world:
/// - Methods that don't alter the world and return values should be prefixed by `get_`, 
///   these are getters and should not usually compute too much, getters that returns
///   mutable reference should be suffixed with `_mut`;
/// - Getter methods that return booleans should prefer `can_`, `has_` or `is_` prefixes;
/// - Methods that alter the world by running a logic tick should start with `tick_`;
/// - Methods that iterate over some world objects should start with `iter_`;
/// - Methods that run on internal events can be prefixed by `handle_`;
/// - All other methods should use a proper verb, preferably composed of one-word to
///   reduce possible meanings (e.g. are `schedule_`, `break_`, `spawn_`, `insert_` or
///   `remove_`).
/// 
/// Various suffixes can be added to methods, depending on the world area affected by the
/// method, for example `_in`, `_in_box` or `_colliding`.
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
    /// Mapping for block entities.
    block_entities: HashMap<IVec3, Box<BlockEntity>>,
    /// Total entities count spawned since the world is running. Also used to give 
    /// entities a unique id.
    entities_count: u32,
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
    /// Total scheduled ticks count since the world is running.
    scheduled_ticks_count: u64,
    /// Mapping of scheduled ticks in the future.
    scheduled_ticks: BTreeSet<ScheduledTick>,
    /// A set of all scheduled tick states, used to avoid ticking twice the same position
    /// and block id. 
    scheduled_ticks_states: HashSet<ScheduledTickState>,
    /// Queue of pending light updates to be processed.
    light_updates: VecDeque<LightUpdate>,
    /// This is the wrapping seed used by random ticks to compute random block positions.
    random_ticks_seed: i32,
    /// Internal cached queue of random ticks that should be computed, this queue is only
    /// used in the random ticking engine. It is put in an option in order to be owned
    /// while random ticking and therefore avoiding borrowing issue with the world.
    random_ticks_pending: Option<Vec<(IVec3, u8, u8)>>,
    /// The current weather in that world, note that the Notchian server do not work like
    /// this, but rather store two independent state for rain and thunder, but we simplify
    /// the logic in this implementation since it is not strictly needed to be on parity.
    weather: Weather,
    /// Next time when the weather should be recomputed.
    weather_next_time: u64,
    /// The current sky light level, depending on the current time. This value is used
    /// when subtracted from a chunk sky light level.
    sky_light_subtracted: u8,
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
            block_entities: HashMap::new(),
            entities_count: 0,
            entities: Vec::new(),
            entities_map: HashMap::new(),
            entities_dead: Vec::new(),
            entities_orphan: IndexSet::new(),
            scheduled_ticks_count: 0,
            scheduled_ticks: BTreeSet::new(),
            scheduled_ticks_states: HashSet::new(),
            light_updates: VecDeque::new(),
            random_ticks_seed: JavaRandom::new_seeded().next_int(),
            random_ticks_pending: Some(Vec::new()),
            weather: Weather::Clear,
            weather_next_time: 0,
            sky_light_subtracted: 0,
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

    /// Get the dimension of this world.
    pub fn get_dimension(&self) -> Dimension {
        self.dimension
    }

    /// Get the world's spawn position.
    pub fn get_spawn_pos(&self) -> DVec3 {
        self.spawn_pos
    }

    /// Set the world's spawn position, this triggers `SpawnPosition` event.
    pub fn set_spawn_pos(&mut self, pos: DVec3) {
        self.spawn_pos = pos;
        self.push_event(Event::SpawnPosition { pos });
    }

    /// Get the world time, in ticks.
    pub fn get_time(&self) -> u64 {
        self.time
    }

    /// Get a mutable access to this world's random number generator.
    pub fn get_rand_mut(&mut self) -> &mut JavaRandom {
        &mut self.rand
    }

    pub fn get_weather(&self) -> Weather {
        self.weather
    }

    pub fn set_weather(&mut self, weather: Weather) {
        if self.weather != weather {
            self.push_event(Event::WeatherChange { prev: self.weather, new: weather });
            self.weather = weather;
        }
    }

    /// Get a reference to a chunk, if existing.
    pub fn get_chunk(&self, cx: i32, cz: i32) -> Option<&Chunk> {
        self.chunks.get(&(cx, cz)).map(|c| &*c.inner)
    }

    /// Get a mutable reference to a chunk, if existing.
    pub fn get_chunk_mut(&mut self, cx: i32, cz: i32) -> Option<&mut Chunk> {
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

    /// Get block and metadata at given position in the world, if the chunk is not
    /// loaded, none is returned.
    /// 
    /// TODO: Work on a world's block cache to speed up access.
    pub fn get_block(&self, pos: IVec3) -> Option<(u8, u8)> {
        let (cx, cz) = calc_chunk_pos(pos)?;
        let chunk = self.get_chunk(cx, cz)?;
        Some(chunk.get_block(pos))
    }

    /// Set block and metadata at given position in the world, if the chunk is not
    /// loaded, none is returned, but if it is existing the previous block and metadata
    /// is returned. This function also push a block change event.
    pub fn set_block(&mut self, pos: IVec3, id: u8, metadata: u8) -> Option<(u8, u8)> {
        
        let (cx, cz) = calc_chunk_pos(pos)?;
        let chunk = self.get_chunk_mut(cx, cz)?;
        let (prev_id, prev_metadata) = chunk.get_block(pos);
        
        if prev_id != id || prev_metadata != metadata {

            chunk.set_block(pos, id, metadata);

            let prev_height = chunk.get_height(pos);
            let height = pos.y as u8 + 1; // Cast is safe because we checked it before.

            if block::material::get_light_opacity(id) != 0 {
                // If the block is opaque and it is placed above current height, update
                // that height to the new one.
                if height > prev_height {
                    chunk.set_height(pos, height);
                }
            } else if prev_height == height {
                // We set a transparent block at the current height, so we need to find 
                // an opaque block below to update height. While we are above 0 we check
                // if the block below is opaque or not.
                let mut check_pos = pos;
                while check_pos.y > 0 {
                    check_pos.y -= 1;
                    let (id, _) = chunk.get_block(check_pos);
                    if block::material::get_light_opacity(id) != 0 {
                        // Increment to the new height just before breaking.
                        check_pos.y += 1;
                        break;
                    }
                }
                // NOTE: If the loop don't find any opaque block below, it is set to 0.
                chunk.set_height(check_pos, check_pos.y as u8);
                
            }

            self.light_updates.push_back(LightUpdate { 
                kind: LightUpdateKind::Block,
                pos,
                credit: 15, // TODO: Use the previous light emission as credit.
            });

            self.light_updates.push_back(LightUpdate { 
                kind: LightUpdateKind::Sky,
                pos,
                credit: 15,
            });

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
        self.notify_remove_unchecked(pos, prev_id, prev_metadata);
        self.notify_add_unchecked(pos, id, metadata);
        Some((prev_id, prev_metadata))
    }

    /// Same as the `set_block_self_notify` method, but additionally the blocks around 
    /// are notified of that neighbor change.
    pub fn set_block_notify(&mut self, pos: IVec3, id: u8, metadata: u8) -> Option<(u8, u8)> {
        let (prev_id, prev_metadata) = self.set_block_self_notify(pos, id, metadata)?;
        self.notify_blocks_around(pos, id);
        Some((prev_id, prev_metadata))
    }

    /// Get saved height of a chunk column.
    pub fn get_height(&self, pos: IVec3) -> Option<u8> {
        let (cx, cz) = calc_chunk_pos(pos)?;
        let chunk = self.get_chunk(cx, cz)?;
        Some(chunk.get_height(pos))
    }

    /// Get light level at the given position, in range 0..16.
    pub fn get_light(&self, mut pos: IVec3, actual_sky_light: bool) -> Option<Light> {
        
        if pos.y > 127 {
            pos.y = 127;
        }

        let (cx, cz) = calc_chunk_pos(pos)?;
        let chunk = self.get_chunk(cx, cz)?;

        // TODO: If stair or farmland, get max value around them.

        let block = chunk.get_block_light(pos);
        let mut sky = chunk.get_sky_light(pos);

        if actual_sky_light {
            sky = sky.saturating_sub(self.sky_light_subtracted);
        }

        Some(Light {
            block,
            sky,
            max: block.max(sky),
        })

    }

    /// Get a block entity from its position.
    /// TODO: Improve API
    pub fn get_block_entity(&self, pos: IVec3) -> Option<&BlockEntity> {
        self.block_entities.get(&pos).map(|b| &**b)
    }

    /// Get a block entity from its position.
    /// TODO: Improve API
    pub fn get_block_entity_mut(&mut self, pos: IVec3) -> Option<&mut BlockEntity> {
        self.block_entities.get_mut(&pos).map(|b| &mut **b)
    }

    /// TODO: Improve API
    pub fn set_block_entity(&mut self, pos: IVec3, block_entity: impl Into<Box<BlockEntity>>) {
        self.block_entities.insert(pos, block_entity.into());
    }

    /// TODO: Improve API
    pub fn remove_block_entity(&mut self, pos: IVec3) -> Option<Box<BlockEntity>> {
        self.block_entities.remove(&pos)
    }

    /// Internal function to ensure monomorphization and reduce bloat of the 
    /// generic [`spawn_entity`].
    #[inline(never)]
    fn spawn_entity_inner(&mut self, mut entity: Box<Entity>) -> u32 {

        // Initial position is used to known in which chunk to cache it.
        let entity_base = entity.base_mut();
        let entity_index = self.entities.len();

        // Get the next unique entity id.
        let id = self.entities_count;
        self.entities_count = self.entities_count.checked_add(1)
            .expect("entity count overflow");

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
    pub fn get_entity(&self, id: u32) -> Option<&Entity> {
        let index = *self.entities_map.get(&id)?;
        Some(self.entities[index].inner
            .as_deref()
            .expect("entity is being updated"))
    }

    /// Get a generic entity from its unique id. This generic entity can later be checked
    /// for being of a particular type.
    #[track_caller]
    pub fn get_entity_mut(&mut self, id: u32) -> Option<&mut Entity> {
        let index = *self.entities_map.get(&id)?;
        Some(self.entities[index].inner
            .as_deref_mut()
            .expect("entity is being updated"))
    }

    /// Schedule a tick update to happen at the given position, for the given block id
    /// and with a given delay in ticks.
    pub fn schedule_tick(&mut self, pos: IVec3, id: u8, delay: u64) {

        let uid = self.scheduled_ticks_count;
        self.scheduled_ticks_count = self.scheduled_ticks_count.checked_add(1)
            .expect("scheduled ticks count overflow");

        let state = ScheduledTickState { pos, id };
        if self.scheduled_ticks_states.insert(state) {
            self.scheduled_ticks.insert(ScheduledTick { time: self.time + delay, state, uid });
        }

    }

    /// Iterate over all blocks in the given area.
    /// *Min is inclusive and max is exclusive.*
    pub fn iter_blocks_in(&self, min: IVec3, max: IVec3) -> impl Iterator<Item = (IVec3, u8, u8)> + '_ {
        BlocksInIter::new(self, min, max)
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
    
    /// Tick the world, this ticks all entities.
    pub fn tick(&mut self) {

        if self.time % 20 == 0 {
            // println!("time: {}", self.time);
            // println!("weather: {:?}", self.weather);
            // println!("weather_next_time: {}", self.weather_next_time);
            // println!("sky_light_subtracted: {}", self.sky_light_subtracted);
        }

        self.tick_weather();
        // TODO: Wake up all sleeping player if day time.
        // TODO: Perform mob spawning.
        
        self.tick_sky_light();

        self.time += 1;

        self.tick_blocks();
        self.tick_entities();
        // TODO: tick block entities.

        self.tick_light();
        
    }

    /// Update current weather in the world.
    fn tick_weather(&mut self) {

        // No weather in the nether.
        if self.dimension == Dimension::Nether {
            return;
        }

        // When it's time to recompute weather.
        if self.time >= self.weather_next_time {

            // Don't update weather on first world tick.
            if self.time != 0 {
                let new_weather = match self.weather {
                    Weather::Clear => self.rand.next_choice(&[Weather::Rain, Weather::Thunder]),
                    _ => self.rand.next_choice(&[self.weather, Weather::Clear]),
                };
                self.set_weather(new_weather);
            }

            let bound = if self.weather == Weather::Clear { 168000 } else { 12000 };
            let delay = self.rand.next_int_bounded(bound) as u64 + 12000;
            self.weather_next_time = self.time + delay;

        }

    }

    /// Update the sky light value depending on the current time, it is then used to get
    /// the real light value of blocks.
    fn tick_sky_light(&mut self) {

        let time_wrapped = self.time % 24000;
        let mut half_turn = (time_wrapped as f32 + 1.0) / 24000.0 - 0.25;

        if half_turn < 0.0 {
            half_turn += 1.0;
        } else if half_turn > 1.0 {
            half_turn -= 1.0;
        }

        let celestial_angle = match self.dimension {
            Dimension::Nether => 0.5,
            _ => half_turn + (1.0 - ((half_turn * std::f32::consts::PI).cos() + 1.0) / 2.0 - half_turn) / 3.0,
        };

        let factor = (celestial_angle * std::f32::consts::TAU).cos() * 2.0 + 0.5;
        let factor = factor.clamp(0.0, 1.0);
        let factor = match self.weather {
            Weather::Clear => 1.0,
            Weather::Rain => 0.6875,
            Weather::Thunder => 0.47265625,
        } * factor;

        self.sky_light_subtracted = ((1.0 - factor) * 11.0) as u8;

    }

    /// Internal function to tick the internal scheduler.
    fn tick_blocks(&mut self) {

        debug_assert_eq!(self.scheduled_ticks.len(), self.scheduled_ticks_states.len());

        // Schedule ticks...
        while let Some(tick) = self.scheduled_ticks.first() {
            if self.time > tick.time {
                // This tick should be activated.
                let tick = self.scheduled_ticks.pop_first().unwrap();
                assert!(self.scheduled_ticks_states.remove(&tick.state));
                // Check coherency of the scheduled tick and current block.
                if let Some((id, metadata)) = self.get_block(tick.state.pos) {
                    if id == tick.state.id {
                        self.tick_block_unchecked(tick.state.pos, id, metadata, false);
                    }
                }
            } else {
                // Our set is ordered by time first, so we break when past current time. 
                break;
            }
        }

        // Random ticking...
        let mut pending_random_ticks = self.random_ticks_pending.take().unwrap();
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

                let (id, metadata) = chunk.inner.get_block(pos);
                pending_random_ticks.push((chunk_pos + pos, id, metadata));

            }

        }

        for (pos, id, metadata) in pending_random_ticks.drain(..) {
            self.tick_block_unchecked(pos, id, metadata, true);
        }

        self.random_ticks_pending = Some(pending_random_ticks);

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

    /// Tick pending light updates.
    fn tick_light(&mut self) {

        // IMPORTANT NOTE: This algorithm is terrible but works, I've been trying to come
        // with a better one but it has been too complicated so far.

        for _ in 0..1000 {

            let Some(update) = self.light_updates.pop_front() else { break };

            let mut max_face_emission = 0;
            for face in Face::ALL {

                let face_pos = update.pos + face.delta();

                let Some((cx, cz)) = calc_chunk_pos(face_pos) else { continue };
                let Some(chunk) = self.chunks.get_mut(&(cx, cz)) else { continue };
                let chunk = &mut *chunk.inner;

                let face_emission = match update.kind {
                    LightUpdateKind::Block => chunk.get_block_light(face_pos),
                    LightUpdateKind::Sky => chunk.get_sky_light(face_pos),
                };

                max_face_emission = max_face_emission.max(face_emission);

            }

            let Some((cx, cz)) = calc_chunk_pos(update.pos) else { continue };
            let Some(chunk) = self.chunks.get_mut(&(cx, cz)) else { continue };
            let chunk = &mut *chunk.inner;

            let (id, _) = chunk.get_block(update.pos);
            let opacity = block::material::get_light_opacity(id).max(1);

            let emission = match update.kind {
                LightUpdateKind::Block => block::material::get_light_emission(id),
                LightUpdateKind::Sky => {
                    // If the block is above ground, then it has
                    let column_height = chunk.get_height(update.pos) as i32;
                    if update.pos.y >= column_height { 15 } else { 0 }
                }
            };

            let new_light = emission.max(max_face_emission.saturating_sub(opacity));
            let mut changed = false;
            let mut sky_exposed = false;

            match update.kind {
                LightUpdateKind::Block => {
                    if chunk.get_block_light(update.pos) != new_light {
                        chunk.set_block_light(update.pos, new_light);
                        changed = true;
                    }
                }
                LightUpdateKind::Sky => {
                    if chunk.get_sky_light(update.pos) != new_light {
                        chunk.set_sky_light(update.pos, new_light);
                        changed = true;
                        sky_exposed = emission == 15;
                    }
                }
            }

            if changed && update.credit >= 1 {
                for face in Face::ALL {
                    // Do not propagate light upward when the updated block is above 
                    // ground, so all blocks above are also exposed and should already
                    // be at max level.
                    if face == Face::PosY && sky_exposed {
                        continue;
                    }
                    self.light_updates.push_back(LightUpdate {
                        kind: update.kind,
                        pos: update.pos + face.delta(),
                        credit: update.credit - 1,
                    });
                }
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

/// Type of weather currently in the world.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Weather {
    /// The weather is clear.
    Clear,
    /// It is raining.
    Rain,
    /// It is thundering.
    Thunder,
}

/// Light value of a position in the world.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Light {
    /// Block light level.
    pub block: u8,
    /// Sky light level, can the absolute sky light or the actual one depending on query.
    pub sky: u8,
    /// Maximum light level between block and sky value.
    pub max: u8,
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
    /// Play the block activation sound at given position and id/metadata.
    BlockSound {
        /// Position of the block to player sound.
        pos: IVec3,
        /// Current id of the block.
        id: u8,
        /// Current metadata of the block.
        metadata: u8,
    },
    /// The world's spawn point has been changed.
    SpawnPosition {
        /// The new spawn point position.
        pos: DVec3,
    },
    WeatherChange {
        prev: Weather,
        new: Weather,
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
#[derive(Eq)]
struct ScheduledTick {
    /// This tick unique id within the world.
    uid: u64,
    /// The time to tick the block.
    time: u64,
    /// State of that scheduled tick.
    state: ScheduledTickState,
}

impl PartialEq for ScheduledTick {
    fn eq(&self, other: &Self) -> bool {
        self.uid == other.uid && self.time == other.time
    }
}

impl PartialOrd for ScheduledTick {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(Ord::cmp(self, other))
    }
}

impl Ord for ScheduledTick {
    fn cmp(&self, other: &Self) -> Ordering {
        self.time.cmp(&other.time)
            .then(self.uid.cmp(&other.uid))
    }
}

/// A light update to apply to the world.
struct LightUpdate {
    /// Light kind targeted by this update, the update only applies to one of the kind.
    kind: LightUpdateKind,
    /// The position of the light update.
    pos: IVec3,
    /// Credit remaining to update light, this is used to limit the number of updates
    /// produced by a block chance initial update. Initial value is something like 15
    /// and decrease for each propagation, when it reaches 0 the light no longer 
    /// propagates.
    credit: u8,
}

/// Different kind of light updates, this affect how the light spread.
#[derive(Clone, Copy, PartialEq, Eq)]
enum LightUpdateKind {
    /// Block light level, the light spread in all directions and blocks have a minimum 
    /// opacity of 1 in all directions.
    Block,
    /// Sky light level, same as block light but light do not decrease when going down.
    Sky,
}


/// An iterator for blocks in a world area. This returns the block id and metadata.
struct BlocksInIter<'a> {
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

impl<'a> BlocksInIter<'a> {

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

        BlocksInIter {
            world,
            chunk: None,
            min,
            max,
            cursor: min,
        }

    }

}

impl<'a> FusedIterator for BlocksInIter<'a> {}
impl<'a> Iterator for BlocksInIter<'a> {

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
                self.chunk = Some((cx, cz, self.world.get_chunk(cx, cz)));
            }
        }

        // If there is no chunk at the position, defaults to (id = 0, metadata = 0).
        let mut ret = (self.cursor, 0, 0);

        // If a chunk exists for the current column.
        if let Some((_, _, Some(chunk))) = self.chunk {
            let (block, metadata) = chunk.get_block(self.cursor);
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
