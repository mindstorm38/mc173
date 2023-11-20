//! Data structure for storing a world (overworld or nether) at runtime.

use std::collections::{HashMap, BTreeSet, HashSet, VecDeque};
use std::ops::{Deref, DerefMut};
use std::iter::FusedIterator;
use std::cmp::Ordering;
use std::cell::Cell;
use std::sync::Arc;
use std::mem;

use glam::{IVec3, Vec2, DVec3};
use indexmap::IndexSet;

use crate::chunk::{Chunk, calc_chunk_pos, calc_chunk_pos_unchecked, calc_entity_chunk_pos, CHUNK_HEIGHT, CHUNK_WIDTH};
use crate::util::{JavaRandom, BoundingBox, Face};
use crate::block_entity::BlockEntity;
use crate::item::ItemStack;
use crate::entity::Entity;
use crate::block;


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


// Various thread local vectors that are used to avoid frequent reallocation of 
// temporary vector used in the logic code.
thread_local! {
    /// This thread local vector is used temporally to stores the random ticks to be 
    /// executed. This is mandatory since ticking a block requires full mutable access to
    /// the world, but it's not possible while owning a reference to a chunk.
    static RANDOM_TICKS_PENDING: Cell<Vec<(IVec3, u8, u8)>> = const { Cell::new(Vec::new()) };
    /// This thread local vector is used to temporally store entities or block entities 
    /// indices that should be removed just after the update loop.
    static INDICES_TO_REMOVE: Cell<Vec<usize>> = const { Cell::new(Vec::new()) };
}


/// This data structure stores different kind of component:
/// - Chunks, these are the storage for block, light and height map of a 16x16 column in
///   the world with a height of 128. This component has the largest memory footprint
///   overall and is stored in shared reference to avoid too much memory copy.
///   A chunk must be present in order to set block in the world.
/// - Entities, basically anything that needs to be ticked with 3 dimensional coordinates.
///   They can control their own position, velocity and look for example.
/// - Block Entities, this is a mix between entities and blocks, they can be ticked but
///   are attached to a block position that they cannot control.
/// 
/// These components are independent, but are internally optimized for access. For example
/// entities are not directly linked to a chunk, but an iterator over entities within a
/// chunk can be obtained.
/// 
/// This data structure is also optimized for actually running the world's logic if 
/// needed. Such as weather, random block ticking, scheduled block ticking, entity 
/// ticking or block notifications.
/// 
/// This structure also allows listening for events within it through a queue of 
/// [`Event`], events listening is disabled by default but can be enabled by swapping
/// a `Vec<Event>` into the world using the [`World::swap_events`]. Events are generated
/// either by world's ticking logic or by manual changes to the world.
/// 
/// This data structure is however not design to handle automatic chunk loading and 
/// saving, every chunk needs to be manually inserted and removed, same for entities and
/// block entities.
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
    /// The mapping of world chunks, with optional world components linked to them, such
    /// as chunk data, entities and block entities. Every world component must be linked
    /// to a world chunk.
    chunks: HashMap<(i32, i32), WorldChunk>,
    /// Total entities count spawned since the world is running. Also used to give 
    /// entities a unique id.
    entities_count: u32,
    /// The internal list of all loaded entities, they are referred to by their index in
    /// this list. This implies that when an entity is moved, all index pointing to it
    /// should be updated to its new index.
    entities: Vec<WorldEntity>,
    /// Entities' index mapping from their unique id.
    entities_id_map: HashMap<u32, usize>,
    /// Same as entities but for block entities.
    block_entities: Vec<WorldBlockEntity>,
    /// Mapping of block entities to they block position.
    block_entities_pos_map: HashMap<IVec3, usize>,
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
            entities_count: 0,
            entities: Vec::new(),
            entities_id_map: HashMap::new(),
            block_entities: Vec::new(),
            block_entities_pos_map: HashMap::new(),
            scheduled_ticks_count: 0,
            scheduled_ticks: BTreeSet::new(),
            scheduled_ticks_states: HashSet::new(),
            light_updates: VecDeque::new(),
            random_ticks_seed: JavaRandom::new_seeded().next_int(),
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
        mem::replace(&mut self.events, events)
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

    /// Get the dimension of this world, this is basically only for sky color on client
    /// and also for celestial angle on the server side for sky light calculation. This
    /// has not direct relation with the actual world generation that is providing this
    /// world with chunks and entities.
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

    /// Get the current weather in the world.
    pub fn get_weather(&self) -> Weather {
        self.weather
    }

    /// Set the current weather in this world. If the weather has changed an event will
    /// be pushed into the events queue.
    pub fn set_weather(&mut self, weather: Weather) {
        if self.weather != weather {
            self.push_event(Event::WeatherChange { prev: self.weather, new: weather });
            self.weather = weather;
        }
    }

    /// Create a snapshot of a chunk's content, this only works if chunk data is existing.
    /// This operation can be costly depending on the number of entities in the chunk, but
    /// is free regarding the block and light data because it use shared reference.
    pub fn take_chunk_snapshot(&self, cx: i32, cz: i32) -> Option<ChunkSnapshot> {
        let world_chunk = self.chunks.get(&(cx, cz))?;
        let chunk = world_chunk.data.as_ref()?;
        Some(ChunkSnapshot {
            cx, 
            cz,
            chunk: Arc::clone(&chunk),
            entities: world_chunk.entities.iter()
                // Ignoring entities being updated, silently for now.
                .filter_map(|&index| self.entities[index].inner.as_ref().cloned())
                .collect(),
            block_entities: world_chunk.block_entities.iter()
                .filter_map(|(&pos, &index)| self.block_entities[index].inner.as_ref()
                    .cloned()
                    .map(|e| (pos, e)))
                .collect(),
        })
    }

    /// Insert a chunk snapshot into this world at its position with all entities and 
    /// block entities attached to it.
    pub fn insert_chunk_snapshot(&mut self, snapshot: ChunkSnapshot) {
        
        self.set_chunk(snapshot.cx, snapshot.cz, snapshot.chunk);
        
        for entity in snapshot.entities {
            debug_assert_eq!(calc_entity_chunk_pos(entity.base().pos), (snapshot.cx, snapshot.cz), "incoherent entity in chunk snapshot");
            self.spawn_entity_inner(entity);
        }

        for (pos, block_entity) in snapshot.block_entities {
            debug_assert_eq!(calc_chunk_pos_unchecked(pos), (snapshot.cx, snapshot.cz), "incoherent block entity in chunk snapshot");
            self.set_block_entity_inner(pos, block_entity);
        }

    }

    /// Get a reference to a chunk, if existing.
    pub fn get_chunk(&self, cx: i32, cz: i32) -> Option<&Chunk> {
        self.chunks.get(&(cx, cz)).and_then(|c| c.data.as_deref())
    }

    /// Get a mutable reference to a chunk, if existing. This operation will clone the 
    /// internal chunk data if it was previously shared in with a [`ChunkView`].
    pub fn get_chunk_mut(&mut self, cx: i32, cz: i32) -> Option<&mut Chunk> {
        self.chunks.get_mut(&(cx, cz)).and_then(|c| c.data.as_mut().map(Arc::make_mut))
    }

    /// Raw function to add a chunk to the world at the given coordinates. Note that the
    /// given chunk only contains block and light data, so no entity or block entity will
    /// be added by this function.
    /// 
    /// If any chunk is existing at this coordinate, it will be replaced and all of its
    /// entities will be transferred to that new chunk.
    /// 
    /// The world allows entities to update outside of actual chunks, such entities are
    /// known as orphan ones. If such entities are currently present at this chunk's
    /// coordinates, they will be moved to this new chunk.
    pub fn set_chunk(&mut self, cx: i32, cz: i32, chunk: Arc<Chunk>) {
        let world_chunk = self.chunks.entry((cx, cz)).or_default();
        let was_unloaded = world_chunk.data.replace(chunk).is_none();
        if was_unloaded {
            for &entity_index in &world_chunk.entities {
                self.entities[entity_index].loaded = true;
            }
            for &block_entity_index in world_chunk.block_entities.values() {
                self.block_entities[block_entity_index].loaded = true;
            }
        }
    }

    /// Remove a chunk that may not exists. Note that this only removed the chunk data,
    /// not its entities and block entities.
    /// 
    /// If the chunk exists, all of its owned entities will be transferred to the orphan 
    /// entities list to be later picked up by another chunk.
    pub fn remove_chunk(&mut self, cx: i32, cz: i32) -> Option<Arc<Chunk>> {
        let world_chunk = self.chunks.get_mut(&(cx, cz))?;
        let ret = world_chunk.data.take();
        if ret.is_some() {
            for &entity_index in &world_chunk.entities {
                self.entities[entity_index].loaded = false;
            }
            for &block_entity_index in world_chunk.block_entities.values() {
                self.block_entities[block_entity_index].loaded = false;
            }
        }
        ret
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
    /// is returned. This function also push a block change event and update lights
    /// accordingly.
    pub fn set_block(&mut self, pos: IVec3, id: u8, metadata: u8) -> Option<(u8, u8)> {

        // println!("set_block({pos}, {id} ({}), {metadata})", block::from_id(id).name);
        
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
        self.notify_change_unchecked(pos, prev_id, prev_metadata, id, metadata);
        Some((prev_id, prev_metadata))
    }

    /// Same as the `set_block_self_notify` method, but additionally the blocks around 
    /// are notified of that neighbor change.
    pub fn set_block_notify(&mut self, pos: IVec3, id: u8, metadata: u8) -> Option<(u8, u8)> {
        let (prev_id, prev_metadata) = self.set_block_self_notify(pos, id, metadata)?;
        self.notify_blocks_around(pos, id);
        Some((prev_id, prev_metadata))
    }

    /// Get saved height of a chunk column, Y component is ignored in the position.
    pub fn get_height(&self, pos: IVec3) -> Option<u8> {
        let (cx, cz) = calc_chunk_pos_unchecked(pos);
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
        
        let (cx, cz) = calc_entity_chunk_pos(entity_base.pos);

        // NOTE: Entities should always be stored in a world chunk.
        let world_chunk = self.chunks.entry((cx, cz)).or_default();
        world_chunk.entities.insert(entity_index);

        let world_entity = WorldEntity {
            inner: InnerStorage::Ready(entity),
            id,
            cx,
            cz,
            loaded: world_chunk.data.is_some(),
            bb: None,
        };
        
        self.entities.push(world_entity);
        self.entities_id_map.insert(id, entity_index);

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
        // NOTE: This method is just a wrapper to erase generics.
        self.spawn_entity_inner(entity.into())
    }

    /// Get a generic entity from its unique id. This generic entity can later be checked
    /// for being of a particular type. None can be returned if no entity is existing for
    /// this id or if the entity is the current entity being updated.
    pub fn get_entity(&self, id: u32) -> Option<&Entity> {
        let index = *self.entities_id_map.get(&id)?;
        self.entities[index].inner.as_deref()
    }

    /// Get a generic entity from its unique id. This generic entity can later be checked
    /// for being of a particular type. None can be returned if no entity is existing for
    /// this id or if the entity is the current entity being updated.
    pub fn get_entity_mut(&mut self, id: u32) -> Option<&mut Entity> {
        let index = *self.entities_id_map.get(&id)?;
        self.entities[index].inner.as_deref_mut()
    }

    /// Remove an entity from its id, returning some boxed entity is successful. This
    /// returns true if the entity has been successfully marked for removal, this function
    /// can be called from any entity updating or not.
    pub fn remove_entity(&mut self, id: u32) -> bool {
        // NOTE: Each entity can be removed once because its ID is removed.
        let Some(index) = self.entities_id_map.remove(&id) else { return false };
        let prev = self.entities[index].inner.replace(InnerStorage::Removed);
        debug_assert!(!matches!(prev, InnerStorage::Removed), "entity should not already be removed");
        self.push_event(Event::EntityRemove { id });
        true
    }

    /// Get a block entity from its position.
    pub fn get_block_entity(&self, pos: IVec3) -> Option<&BlockEntity> {
        let index = *self.block_entities_pos_map.get(&pos)?;
        self.block_entities[index].inner.as_deref()
    }

    /// Get a block entity from its position.
    pub fn get_block_entity_mut(&mut self, pos: IVec3) -> Option<&mut BlockEntity> {
        let index = *self.block_entities_pos_map.get(&pos)?;
        self.block_entities[index].inner.as_deref_mut()
    }

    /// Inner function to set block entity at given position, used to elide generics.
    #[inline(never)]
    fn set_block_entity_inner(&mut self, pos: IVec3, block_entity: Box<BlockEntity>) {

        let block_entity_index = self.block_entities.len();

        if let Some(prev_index) = self.block_entities_pos_map.insert(pos, block_entity_index) {
            // If a block entity was already present at this position, mark the previous
            // one as removed in order to clean it up later.
            self.remove_block_entity_inner(prev_index);
        }

        let (cx, cz) = calc_chunk_pos_unchecked(pos);
        let world_chunk = self.chunks.entry((cx, cz)).or_default();

        world_chunk.block_entities.insert(pos, block_entity_index);

        let world_block_entity = WorldBlockEntity { 
            inner: InnerStorage::Ready(block_entity), 
            loaded: world_chunk.data.is_some(),
            pos,
        };

        self.block_entities.push(world_block_entity);
        self.push_event(Event::BlockEntitySet { pos });

    }

    /// Set the block entity at the given position. If a block entity was already at the
    /// position, it is removed silently.
    #[inline(always)]
    pub fn set_block_entity(&mut self, pos: IVec3, block_entity: impl Into<Box<BlockEntity>>) {
        self.set_block_entity_inner(pos, block_entity.into());
    }

    /// Internal function to mark the block entity at index as removed.
    fn remove_block_entity_inner(&mut self, index: usize) {
        let world_block_entity = &mut self.block_entities[index];
        let prev = world_block_entity.inner.replace(InnerStorage::Removed);
        debug_assert!(!matches!(prev, InnerStorage::Removed), "block entity should not already be removed");
        let pos = world_block_entity.pos;
        self.push_event(Event::BlockEntityRemove { pos });
    }

    /// Remove a block entity from a position. Returning true if successful.
    pub fn remove_block_entity(&mut self, pos: IVec3) -> bool {
        let Some(index) = self.block_entities_pos_map.remove(&pos) else { return false };
        self.remove_block_entity_inner(index);
        true
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
        self.chunks.get(&(cx, cz))
            .into_iter() // Iterate the option, so if the chunk is absent, nothing return.
            .flat_map(|world_chunk| world_chunk.entities.iter())
            .map(move |&entity_index| &self.entities[entity_index])
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
                    if let (InnerStorage::Ready(entity), Some(entity_bb)) = (&entity.inner, entity.bb) {
                        bb.intersects(entity_bb).then_some((&**entity, entity_bb))
                    } else {
                        None
                    }
                })
            })

    }
    
    /// Tick the world, this ticks all entities.
    /// TODO: Guard this from being called recursively from tick functions.
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
        self.tick_block_entities();

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
        let mut pending_random_ticks = RANDOM_TICKS_PENDING.take();
        debug_assert!(pending_random_ticks.is_empty());

        // Random tick only on loaded chunks.
        for (&(cx, cz), chunk) in &mut self.chunks {
            if let Some(chunk_data) = &chunk.data {

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

                    let (id, metadata) = chunk_data.get_block(pos);
                    pending_random_ticks.push((chunk_pos + pos, id, metadata));

                }

            }
        }

        for (pos, id, metadata) in pending_random_ticks.drain(..) {
            self.tick_block_unchecked(pos, id, metadata, true);
        }

        RANDOM_TICKS_PENDING.set(pending_random_ticks);

    }

    /// Internal function to tick all entities.
    fn tick_entities(&mut self) {

        let mut indices_to_remove = INDICES_TO_REMOVE.take();
        debug_assert!(indices_to_remove.is_empty());

        // Update every entity's bounding box prior to actually ticking.
        for world_entity in &mut self.entities {
            // Ignore removed entities.
            if let InnerStorage::Ready(entity) = &world_entity.inner {
                let entity_base = entity.base();
                world_entity.bb = entity_base.coherent.then_some(entity_base.bb);
            }
        }

        // NOTE: Only update the entities that are present at the start of ticking.
        for entity_index in 0..self.entities.len() {

            // We unwrap because all entities should be present except updated one.
            let world_entity = &mut self.entities[entity_index];
            let mut entity = match world_entity.inner {
                InnerStorage::Removed => {
                    // The entity has been removed, we definitely remove it here.
                    indices_to_remove.push(entity_index);
                    continue;
                }
                InnerStorage::Updated => panic!("entity was already being updated"),
                InnerStorage::Ready(_) => {
                    // Do not update the entity if its chunk is not loaded.
                    if !world_entity.loaded {
                        continue;
                    }
                    // If the entity is ready for update, set 'Updated' state.
                    match world_entity.inner.replace(InnerStorage::Updated) {
                        InnerStorage::Ready(data) => data,
                        _ => unreachable!()
                    }
                }
            };

            // Store the previous chunk, used when comparing with new chunk.
            let (prev_cx, prev_cz) = (world_entity.cx, world_entity.cz);
            entity.tick(&mut *self);

            // Entity is not dead so we re-insert its 
            let world_entity = &mut self.entities[entity_index];
            world_entity.loaded = true;
            match world_entity.inner {
                InnerStorage::Ready(_) => panic!("entity should not be ready"),
                InnerStorage::Removed => {
                    // Entity has been removed while ticking.
                    indices_to_remove.push(entity_index);
                    continue;
                }
                InnerStorage::Updated => {}
            }

            // Before re-adding the entity, check dirty flags to send proper events.
            let entity_base = entity.base_mut();

            // Take all dirty flags.
            let pos_dirty = mem::take(&mut entity_base.pos_dirty);
            let look_dirty = mem::take(&mut entity_base.look_dirty);
            let vel_dirty = mem::take(&mut entity_base.vel_dirty);
            
            let new_chunk = pos_dirty.then_some(calc_entity_chunk_pos(entity_base.pos));

            if let Some(events) = &mut self.events {
                if pos_dirty {
                    events.push(Event::EntityPosition { id: entity_base.id, pos: entity_base.pos });
                }
                if look_dirty {
                    events.push(Event::EntityLook { id: entity_base.id, look: entity_base.look });
                }
                if vel_dirty {
                    events.push(Event::EntityVelocity { id: entity_base.id, vel: entity_base.vel });
                }
            }

            // Entity is still in updated state as expected.
            world_entity.inner = InnerStorage::Ready(entity);

            // Check if the entity moved to another chunk...
            if let Some((new_cx, new_cz)) = new_chunk {
                if (prev_cx, prev_cz) != (new_cx, new_cz) {

                    let remove_success = self.chunks.get_mut(&(prev_cx, prev_cz))
                        .expect("entity previous chunk is missing")
                        .entities
                        .remove(&entity_index);
                    
                    debug_assert!(remove_success, "entity index not found in previous chunk");

                    // Update the world entity to its new chunk and orphan state.
                    world_entity.cx = new_cx;
                    world_entity.cz = new_cz;

                    // Insert the entity in its new chunk.
                    let new_world_chunk = self.chunks.entry((new_cx, new_cz)).or_default();
                    new_world_chunk.entities.insert(entity_index);
                    // Update the loaded flag of the entity depending on the new chunk
                    // being loaded or not.
                    world_entity.loaded = new_world_chunk.data.is_some();

                }
            }

        }

        // It's really important to understand that indices to remove have been pushed
        // from lower index to higher (because of iteration order). So we need to drain
        // in reverse in order to remove the last indices first, because of the swap we
        // are guaranteed that the swapped entity will not be in the indices to remove.
        for index_to_remove in indices_to_remove.drain(..).rev() {

            let removed_entity = self.entities.swap_remove(index_to_remove);
            
            // Try to remove the entity from its chunk, if possible. Entity may no longer
            // be present in its chunk because it 
            if let Some(prev_world_chunk) = self.chunks.get_mut(&(removed_entity.cx, removed_entity.cz)) {
                prev_world_chunk.entities.remove(&index_to_remove);
            }
            
            // The only way to remove an entity is using the `remove_entity` method that
            // also remove the mapping from id to entity index.
            debug_assert!(!self.entities_id_map.contains_key(&removed_entity.id), "removed entity should no longer be mapped");
            
            // Because we used swap remove, this may have moved the last entity (if
            // existing) to the removed entity index. We need to update its index in 
            // chunk or orphan entities.
            if let Some(swapped_entity) = self.entities.get(index_to_remove) {
                
                let entities = &mut self.chunks.get_mut(&(swapped_entity.cx, swapped_entity.cz))
                    .expect("entity was not in a chunk")
                    .entities;
            
                // The swapped entity was at the end, so the new length.
                let previous_index = self.entities.len();
                
                // Update the mapping from entity unique id to the new index.
                let previous_map_index = self.entities_id_map.insert(swapped_entity.id, index_to_remove);
                debug_assert_eq!(previous_map_index, Some(previous_index), "incoherent previous entity index");
            
                let remove_success = entities.remove(&previous_index);
                debug_assert!(remove_success, "entity index not found where it belongs");
                entities.insert(index_to_remove);
                
            }
            
        }

        INDICES_TO_REMOVE.set(indices_to_remove);

    }

    fn tick_block_entities(&mut self) {

        // let mut indices_to_remove = INDICES_TO_REMOVE.take();
        // debug_assert!(indices_to_remove.is_empty());

        // for block_entity_index in 0..self.block_entities.len() {

        //     let world_block_entity = &mut self.entities[block_entity_index];

        // }

        // TODO:

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
                let Some(chunk) = self.get_chunk_mut(cx, cz) else { continue };

                let face_emission = match update.kind {
                    LightUpdateKind::Block => chunk.get_block_light(face_pos),
                    LightUpdateKind::Sky => chunk.get_sky_light(face_pos),
                };

                max_face_emission = max_face_emission.max(face_emission);

            }

            let Some((cx, cz)) = calc_chunk_pos(update.pos) else { continue };
            let Some(chunk) = self.get_chunk_mut(cx, cz) else { continue };

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
    EntityRemove {
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
    /// A block entity has been set at this position.
    BlockEntitySet {
        /// The position of the new block entity.
        pos: IVec3,
    },
    /// A block entity has been remove from this position.
    BlockEntityRemove {
        /// The position of the remove block entity.
        pos: IVec3,
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

/// A snapshot contains all of the content within a chunk, block, light, height map,
/// entities and block entities are all included. This structure can be considered as
/// a "view" because the chunk data (around 80 KB) is referenced to with a [`Arc`], that
/// allows either uniquely owning it, or sharing it with a world, which is the case when
/// saving a chunk.
#[derive(Clone)]
pub struct ChunkSnapshot {
    /// The X chunk coordinate.
    pub cx: i32,
    /// The Z chunk coordinate.
    pub cz: i32,
    /// The block, light and height map data of the chunk.
    pub chunk: Arc<Chunk>,
    /// The entities in that chunk, note that entities are not guaranteed to have a 
    /// position that is within chunk boundaries.
    pub entities: Vec<Box<Entity>>,
    /// Block entities in that chunk, all block entities are mapped to their absolute
    /// coordinates in the world.
    pub block_entities: HashMap<IVec3, Box<BlockEntity>>,
}

impl ChunkSnapshot {

    /// Create a new empty chunk view of the given coordinates.
    pub fn new(cx: i32, cz: i32) -> Self {
        Self {
            cx,
            cz,
            chunk: Chunk::new(),
            entities: Vec::new(),
            block_entities: HashMap::new(),
        }
    }

}


/// This internal structure is used to keep data associated to a chunk coordinate X/Z.
/// It could store chunk data, entities and block entities when present. If a world chunk
/// does not contain data, it is considered **unloaded**. It is also impossible to get
/// a snapshot of an unloaded chunk.
/// 
/// Entities and block entities in **unloaded** chunks are no longer updated as soon as
/// they enter that unloaded chunk.
#[derive(Default)]
struct WorldChunk {
    /// Underlying chunk. This is important to understand why the data chunk is stored 
    /// in an Atomically Reference-Counted container: first the chunk structure is large
    /// (around 80 KB) so we want it be stored in heap while the Arc container allows us
    /// to work with the chunk in a Clone-On-Write manner.
    /// 
    /// In normal conditions, this chunk will not be shared and so it could be mutated 
    /// using the [`Arc::get_mut`] method that allows mutating the Arc's value if only
    /// one reference exists. But there are situations when we want to have more 
    /// references to that chunk data, for example when saving the chunk we'll temporarily
    /// create a Arc referencing this chunk and pass it to the threaded loader/saver.
    /// If the chunk is mutated while being saved, we'll just clone it and replace this
    /// Arc with a new one that, by definition, has only one reference, all of this based
    /// on the [`Arc::make_mut`] method. Depending on save being fast or not, this clone
    /// will be more or less likely to happen.
    data: Option<Arc<Chunk>>,
    /// Entities belonging to this chunk.
    entities: IndexSet<usize>,
    /// Block entities belonging to this chunk.
    block_entities: HashMap<IVec3, usize>,
}

/// Internal type for storing a world entity and keep track of its current chunk.
struct WorldEntity {
    /// The entity storage.
    inner: InnerStorage<Box<Entity>>,
    /// Unique entity id is duplicated here to allow us to access it event when entity
    /// is updating.
    id: u32,
    /// The last computed chunk position X.
    cx: i32,
    /// The last computed chunk position Z.
    cz: i32,
    /// True when the chunk this entity is in is loaded with data.
    loaded: bool,
    /// The bounding box of this entity prior to ticking, none is used if the entity
    /// bounding box isn't coherent, which is the default when the entity has just been
    /// spawned.
    bb: Option<BoundingBox>,
}

/// Internal type for storing a world block entity.
struct WorldBlockEntity {
    /// The block entity storage.
    inner: InnerStorage<Box<BlockEntity>>,
    /// True when the chunk this block entity is in is loaded with data.
    loaded: bool,
    /// Position of that block entity.
    pos: IVec3,
}

/// State of an entity or block entity storage.
enum InnerStorage<T> {
    /// The entity is present and ready to update.
    Ready(T),
    /// The entity storage is temporally owned by the tick function in order to update it.
    Updated,
    /// The entity has been marked for removal and will be removed.
    Removed,
}

impl<T> InnerStorage<T> {

    /// If the inner storage is ready for update, return some shared reference to it.
    #[inline]
    fn as_ref(&self) -> Option<&T> {
        match self {
            Self::Ready(data) => Some(data),
            _ => None
        }
    }
    
    /// If the inner storage data is ready and is [`Deref`], its target is returned.
    #[inline]
    fn as_deref(&self) -> Option<&T::Target>
    where
        T: Deref
    {
        match self {
            Self::Ready(data) => Some(data.deref()),
            _ => None
        }
    }

    /// If the inner storage data is ready and is [`DerefMut`], its target is returned.
    #[inline]
    fn as_deref_mut(&mut self) -> Option<&mut T::Target>
    where
        T: DerefMut,
    {
        match self {
            Self::Ready(data) => Some(data.deref_mut()),
            _ => None
        }
    }

    /// Replace that storage with another one, returning the previous one.
    #[inline]
    fn replace(&mut self, value: Self) -> Self {
        mem::replace(self, value)
    }

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
