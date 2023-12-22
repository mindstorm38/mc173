//! Data structure for storing a world (overworld or nether) at runtime.

use std::collections::{HashMap, BTreeSet, HashSet, VecDeque};
use std::ops::{Deref, DerefMut};
use std::iter::FusedIterator;
use std::cmp::Ordering;
use std::hash::Hash;
use std::cell::Cell;
use std::sync::Arc;
use std::slice;
use std::mem;

use glam::{IVec3, Vec2, DVec3};
use indexmap::IndexSet;

use tracing::{trace, instrument};

use crate::biome::Biome;
use crate::chunk::{Chunk, 
    calc_chunk_pos, calc_chunk_pos_unchecked, calc_entity_chunk_pos,
    CHUNK_HEIGHT, CHUNK_WIDTH};
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
pub mod r#use;
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


/// # Components 
/// 
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
/// This data structure is however not designed to handle automatic chunk loading and 
/// saving, every chunk needs to be manually inserted and removed, same for entities and
/// block entities.
/// 
/// # Logic
/// 
/// This data structure is also optimized for actually running the world's logic if 
/// needed. Such as weather, random block ticking, scheduled block ticking, entity 
/// ticking or block notifications.
/// 
/// # Events
/// 
/// This structure also allows listening for events within it through a queue of 
/// [`Event`], events listening is disabled by default but can be enabled by swapping
/// a `Vec<Event>` into the world using the [`World::swap_events`]. Events are generated
/// either by world's ticking logic or by manual changes to the world.
/// 
/// # Naming convention
/// 
/// Methods provided on this structure should follow a naming convention depending on the
/// action that will apply to the world:
/// - Methods that don't alter the world and return values should be prefixed by `get_`, 
///   these are getters and should not usually compute too much, getters that returns
///   mutable reference should be suffixed with `_mut`;
/// - Getter methods that return booleans should prefer `can_`, `has_` or `is_` prefixes;
/// - Methods that alter the world by running a logic tick should start with `tick_`;
/// - Methods that iterate over some world objects should start with `iter_`, the return
///   iterator type should preferably be a new type (not `impl Iterator`);
/// - Methods that run on internal events can be prefixed by `handle_`;
/// - All other methods should use a proper verb, preferably composed of one-word to
///   reduce possible meanings (e.g. are `schedule_`, `break_`, `spawn_`, `insert_` or
///   `remove_`).
/// 
/// Various suffixes can be added to methods, depending on the world area affected by the
/// method, for example `_in`, `_in_chunk`, `_in_box` or `_colliding`.
/// Any mutation prefix `_mut` should be placed at the very end.
/// 
/// # Roadmap
/// 
/// - Make a diagram to better explain the world structure with entity caching.
/// - Immediate entity/block entity removal if not ticking.
#[derive(Clone)]
pub struct World {
    /// When enabled, this contains the list of events that happened in the world since
    /// it was last swapped. This swap behavior is really useful in order to avoid 
    /// borrowing issues, by temporarily taking ownership of events, the caller can get
    /// a mutable reference to that world at the same time.
    events: Option<Vec<Event>>,
    /// The dimension
    dimension: Dimension,
    /// The world time, increasing on each tick. This is used for day/night cycle but 
    /// also for registering scheduled ticks.
    time: u64,
    /// The world's global random number generator, it is used everywhere to randomize
    /// events in the world, such as plant grow.
    rand: JavaRandom,
    /// The mapping of world chunks, with optional world components linked to them, such
    /// as chunk data, entities and block entities. Every world component must be linked
    /// to a world chunk.
    chunks: HashMap<(i32, i32), ChunkComponent>,
    /// Total entities count spawned since the world is running. Also used to give 
    /// entities a unique id.
    entities_count: u32,
    /// The internal list of all loaded entities, they are referred to by their index in
    /// this list. This implies that when an entity is moved, all index pointing to it
    /// should be updated to its new index.
    entities: Vec<EntityComponent>,
    /// Entities' index mapping from their unique id.
    entities_id_map: HashMap<u32, usize>,
    /// Same as entities but for block entities.
    block_entities: Vec<BlockEntityComponent>,
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
            self.push_event(Event::Weather { prev: self.weather, new: weather });
            self.weather = weather;
        }
    }

    // =================== //
    //   CHUNK SNAPSHOTS   //
    // =================== //

    /// Insert a chunk snapshot into this world at its position with all entities and 
    /// block entities attached to it.
    pub fn insert_chunk_snapshot(&mut self, snapshot: ChunkSnapshot) {
        
        self.set_chunk(snapshot.cx, snapshot.cz, snapshot.chunk);
        
        for entity in snapshot.entities {
            debug_assert_eq!(calc_entity_chunk_pos(entity.0.pos), (snapshot.cx, snapshot.cz), "incoherent entity in chunk snapshot");
            self.spawn_entity_inner(entity);
        }

        for (pos, block_entity) in snapshot.block_entities {
            debug_assert_eq!(calc_chunk_pos_unchecked(pos), (snapshot.cx, snapshot.cz), "incoherent block entity in chunk snapshot");
            self.set_block_entity_inner(pos, block_entity);
        }

    }

    /// Create a snapshot of a chunk's content, this only works if chunk data is existing.
    /// This operation can be costly depending on the number of entities in the chunk, but
    /// is free regarding the block and light data because it use shared reference.
    pub fn take_chunk_snapshot(&self, cx: i32, cz: i32) -> Option<ChunkSnapshot> {
        let chunk_comp = self.chunks.get(&(cx, cz))?;
        let chunk = chunk_comp.data.as_ref()?;
        Some(ChunkSnapshot {
            cx, 
            cz,
            chunk: Arc::clone(&chunk),
            entities: chunk_comp.entities.iter()
                // Ignoring entities being updated, silently for now.
                .filter_map(|&index| self.entities[index].inner.as_ref().cloned())
                .collect(),
            block_entities: chunk_comp.block_entities.iter()
                .filter_map(|(&pos, &index)| self.block_entities[index].inner.as_ref()
                    .cloned()
                    .map(|e| (pos, e)))
                .collect(),
        })
    }

    /// Remove a chunk at given chunk coordinates and return a snapshot of it. If there
    /// is no chunk at the coordinates but entities or block entities are present, None
    /// is returned but entities and block entities are removed from the world.
    pub fn remove_chunk_snapshot(&mut self, cx: i32, cz: i32) -> Option<ChunkSnapshot> {
        
        let chunk_comp = self.chunks.remove(&(cx, cz))?;
        let mut ret = None;

        if let Some(chunk) = chunk_comp.data {
            ret = Some(ChunkSnapshot { 
                cx, 
                cz,
                chunk,
                entities: chunk_comp.entities.iter()
                    // Ignoring entities being updated, silently for now.
                    .filter_map(|&index| self.entities[index].inner.as_ref().cloned())
                    .collect(),
                block_entities: chunk_comp.block_entities.iter()
                    .filter_map(|(&pos, &index)| self.block_entities[index].inner.as_ref()
                        .cloned()
                        .map(|e| (pos, e)))
                    .collect(),
            });
        }

        for index in chunk_comp.entities {
            let comp = &mut self.entities[index];
            let prev = comp.inner.replace(ComponentStorage::Removed);
            debug_assert!(!matches!(prev, ComponentStorage::Removed), "entity should not already be removed");
            self.entities_id_map.remove(&comp.id);
        }

        for (pos, index) in chunk_comp.block_entities {
            let prev = self.block_entities[index].inner.replace(ComponentStorage::Removed);
            debug_assert!(!matches!(prev, ComponentStorage::Removed), "block entity should not already be removed");
            self.block_entities_pos_map.remove(&pos);
        }

        ret

    }

    // =================== //
    //        CHUNKS       //
    // =================== //

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
        let chunk_comp = self.chunks.entry((cx, cz)).or_default();
        let was_unloaded = chunk_comp.data.replace(chunk).is_none();
        if was_unloaded {
            for &entity_index in &chunk_comp.entities {
                self.entities[entity_index].loaded = true;
            }
            for &block_entity_index in chunk_comp.block_entities.values() {
                self.block_entities[block_entity_index].loaded = true;
            }
        }
    }

    /// Return true if a given chunk is present in the world.
    pub fn contains_chunk(&self, cx: i32, cz: i32) -> bool {
        self.chunks.get(&(cx, cz)).is_some_and(|c| c.data.is_some())
    }

    /// Get a reference to a chunk, if existing.
    pub fn get_chunk(&self, cx: i32, cz: i32) -> Option<&Chunk> {
        self.chunks.get(&(cx, cz)).and_then(|c| c.data.as_deref())
    }

    /// Get a mutable reference to a chunk, if existing.
    pub fn get_chunk_mut(&mut self, cx: i32, cz: i32) -> Option<&mut Chunk> {
        self.chunks.get_mut(&(cx, cz)).and_then(|c| c.data.as_mut().map(Arc::make_mut))
    }

    /// Remove a chunk that may not exists. Note that this only removed the chunk data,
    /// not its entities and block entities.
    pub fn remove_chunk(&mut self, cx: i32, cz: i32) -> Option<Arc<Chunk>> {
        let chunk_comp = self.chunks.get_mut(&(cx, cz))?;
        let ret = chunk_comp.data.take();
        if ret.is_some() {
            for &entity_index in &chunk_comp.entities {
                self.entities[entity_index].loaded = false;
            }
            for &block_entity_index in chunk_comp.block_entities.values() {
                self.block_entities[block_entity_index].loaded = false;
            }
        }
        ret
    }

    // =================== //
    //        BLOCKS       //
    // =================== //

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
            chunk.recompute_height(pos);

            // TODO: Move light update to self_notify function to avoid light updates in
            // chunk generation.

            self.light_updates.push_back(LightUpdate { 
                kind: LightUpdateKind::Block,
                pos,
                credit: 15,
            });

            self.light_updates.push_back(LightUpdate { 
                kind: LightUpdateKind::Sky,
                pos,
                credit: 15,
            });

            self.push_event(Event::Block { 
                pos, 
                inner: BlockEvent::Set {
                    id, 
                    metadata,
                    prev_id, 
                    prev_metadata, 
                } 
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

    /// Get block and metadata at given position in the world, if the chunk is not
    /// loaded, none is returned.
    pub fn get_block(&self, pos: IVec3) -> Option<(u8, u8)> {
        let (cx, cz) = calc_chunk_pos(pos)?;
        let chunk = self.get_chunk(cx, cz)?;
        Some(chunk.get_block(pos))
    }

    // =================== //
    //        HEIGHT       //
    // =================== //

    /// Get saved height of a chunk column, Y component is ignored in the position.
    pub fn get_height(&self, pos: IVec3) -> Option<u8> {
        let (cx, cz) = calc_chunk_pos_unchecked(pos);
        let chunk = self.get_chunk(cx, cz)?;
        Some(chunk.get_height(pos))
    }

    // =================== //
    //        LIGHTS       //
    // =================== //

    /// Get light level at the given position, in range 0..16.
    /// 
    /// TODO: Maybe always return light, with default value if chunk is absent.
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

    /// Compute the client-side brightness of a block, based on its block light level.
    pub fn get_brightness(&self, pos: IVec3) -> Option<f32> {
        let block_light = self.get_light(pos, false)?.block;
        let base = 1.0 - block_light as f32 / 15.0;
        let brightness = (1.0 - base) * (base * 3.0 + 1.0) * (1.0 - 0.05) + 0.05;
        Some(brightness)
    }

    // =================== //
    //        BIOMES       //
    // =================== //

    /// Get the biome at some position (Y component is ignored).
    pub fn get_biome(&self, pos: IVec3) -> Option<Biome> {
        let (cx, cz) = calc_chunk_pos_unchecked(pos);
        let chunk = self.get_chunk(cx, cz)?;
        Some(chunk.get_biome(pos))
    }

    // =================== //
    //       ENTITIES      //
    // =================== //

    /// Internal function to ensure monomorphization and reduce bloat of the 
    /// generic [`spawn_entity`].
    #[inline(never)]
    fn spawn_entity_inner(&mut self, entity: Box<Entity>) -> u32 {

        // Initial position is used to known in which chunk to cache it.
        let entity_index = self.entities.len();

        // Get the next unique entity id.
        let id = self.entities_count;
        self.entities_count = self.entities_count.checked_add(1)
            .expect("entity count overflow");

        trace!("spawn entity #{id} ({:?})", entity.kind());

        let (cx, cz) = calc_entity_chunk_pos(entity.0.pos);

        // NOTE: Entities should always be stored in a world chunk.
        let chunk_comp = self.chunks.entry((cx, cz)).or_default();
        chunk_comp.entities.insert(entity_index);

        let entity_comp = EntityComponent {
            inner: ComponentStorage::Ready(entity),
            id,
            cx,
            cz,
            loaded: chunk_comp.data.is_some(),
        };
        
        self.entities.push(entity_comp);
        self.entities_id_map.insert(id, entity_index);

        self.push_event(Event::Entity { id, inner: EntityEvent::Spawn });
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

    /// Return true if an entity is present from its id.
    pub fn contains_entity(&self, id: u32) -> bool {
        self.entities_id_map.contains_key(&id)
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

    /// Remove an entity with given id, returning some boxed entity is successful. This
    /// returns true if the entity has been successfully removed removal, the entity's
    /// storage is guaranteed to be freed after return, but the entity footprint in the
    /// world will be cleaned only after ticking.
    pub fn remove_entity(&mut self, id: u32) -> bool {
        
        // NOTE: Each entity can be removed once because ID is removed with it.
        let Some(index) = self.entities_id_map.remove(&id) else { return false };
        let entity_comp = &mut self.entities[index];
        let prev = entity_comp.inner.replace(ComponentStorage::Removed);
        debug_assert!(!matches!(prev, ComponentStorage::Removed), "entity should not already be removed");
        
        // Directly remove the entity from its chunk.
        let removed_success = self.chunks.get_mut(&(entity_comp.cx, entity_comp.cz))
            .expect("entity chunk is missing")
            .entities.remove(&index);

        debug_assert!(removed_success, "entity missing from its chunk");

        trace!("remove entity #{id}");

        self.push_event(Event::Entity { id, inner: EntityEvent::Remove });
        true

    }

    // =================== //
    //   BLOCK ENTITIES    //
    // =================== //

    /// Inner function to set block entity at given position, used to elide generics.
    #[inline(never)]
    fn set_block_entity_inner(&mut self, pos: IVec3, block_entity: Box<BlockEntity>) {

        // This is the future index where we insert the block entity.
        let block_entity_index = self.block_entities.len();

        if let Some(prev_index) = self.block_entities_pos_map.insert(pos, block_entity_index) {
            // If a block entity was already present at this position, mark the previous
            // one as removed in order to clean it up later.
            self.remove_block_entity_inner(prev_index);
        }

        let (cx, cz) = calc_chunk_pos_unchecked(pos);
        let chunk_comp = self.chunks.entry((cx, cz)).or_default();

        chunk_comp.block_entities.insert(pos, block_entity_index);

        let block_entity_comp = BlockEntityComponent { 
            inner: ComponentStorage::Ready(block_entity), 
            loaded: chunk_comp.data.is_some(),
            pos,
        };

        self.block_entities.push(block_entity_comp);
        self.push_event(Event::BlockEntity { pos, inner: BlockEntityEvent::Set });

    }

    /// Set the block entity at the given position. If a block entity was already at the
    /// position, it is removed silently.
    #[inline(always)]
    pub fn set_block_entity(&mut self, pos: IVec3, block_entity: impl Into<Box<BlockEntity>>) {
        self.set_block_entity_inner(pos, block_entity.into());
    }

    /// Return true if some block entity is present in the world.
    pub fn contains_block_entity(&self, pos: IVec3) -> bool {
        self.block_entities_pos_map.contains_key(&pos)
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

    /// Internal function to mark the block entity at index as removed.
    fn remove_block_entity_inner(&mut self, index: usize) {
        
        let block_entity_comp = &mut self.block_entities[index];
        let prev = block_entity_comp.inner.replace(ComponentStorage::Removed);
        debug_assert!(!matches!(prev, ComponentStorage::Removed), "block entity should not already be removed");
        
        // Directly remove the block entity from its chunk.
        let pos = block_entity_comp.pos;
        let (cx, cz) = calc_chunk_pos_unchecked(pos);
        let remove_success = self.chunks.get_mut(&(cx, cz))
            .expect("block entity chunk is missing")
            .block_entities.remove(&pos)
            .is_some();

        debug_assert!(remove_success, "block entity missing from its chunk");

        self.push_event(Event::BlockEntity { pos, inner: BlockEntityEvent::Remove });
    
    }

    /// Remove a block entity from a position. Returning true if successful, in this case
    /// the block entity storage is guaranteed to be freed, but the block entity footprint
    /// in this world will be definitely cleaned after ticking.
    pub fn remove_block_entity(&mut self, pos: IVec3) -> bool {
        let Some(index) = self.block_entities_pos_map.remove(&pos) else { return false };
        self.remove_block_entity_inner(index);
        true
    }

    // =================== //
    //   SCHEDULED TICKS   //
    // =================== //

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

    // =================== //
    //      ITERATORS      //
    // =================== //

    /// Iterate over all blocks in the given area where max is excluded.
    #[inline]
    pub fn iter_blocks_in(&self, min: IVec3, max: IVec3) -> BlocksInIter<'_> {
        BlocksInIter::new(self, min, max)
    }

    /// Iterate over all blocks in the chunk at given coordinates.
    #[inline]
    pub fn iter_blocks_in_chunk(&self, cx: i32, cz: i32) -> BlocksInChunkIter<'_> {
        BlocksInChunkIter::new(self, cx, cz)
    }

    /// Iterate over all entities in the world. The currently updated entity is not 
    /// included in this iterator.
    #[inline]
    pub fn iter_entities(&self) -> EntitiesIter<'_> {
        EntitiesIter(self.entities.iter())
    }

    /// Iterator over all entities in the world through mutable references.
    #[inline]
    pub fn iter_entities_mut(&mut self) -> EntitiesIterMut<'_> {
        EntitiesIterMut(self.entities.iter_mut())
    }

    /// Iterate over all entities of the given chunk.
    /// *This function can't return the current updated entity.*
    #[inline]
    pub fn iter_entities_in_chunk(&self, cx: i32, cz: i32) -> EntitiesInChunkIter<'_> {
        EntitiesInChunkIter {
            indices: self.chunks.get(&(cx, cz)).map(|comp| comp.entities.iter()),
            entities: &self.entities[..],
        }
    }

    /// Iterate over all entities of the given chunk through mutable references.
    /// *This function can't return the current updated entity.*
    #[inline]
    pub fn iter_entities_in_chunk_mut(&mut self, cx: i32, cz: i32) -> EntitiesInChunkIterMut<'_> {
        EntitiesInChunkIterMut {
            indices: self.chunks.get(&(cx, cz)).map(|comp| comp.entities.iter()),
            entities: &mut self.entities[..],
            #[cfg(debug_assertions)]
            returned_pointers: HashSet::new(),
        }
    }

    /// Iterate over all entities colliding with the given bounding box.
    /// *This function can't return the current updated entity.*
    #[inline]
    pub fn iter_entities_colliding(&self, bb: BoundingBox) -> EntitiesCollidingIter<'_> {

        let (start_cx, start_cz) = calc_entity_chunk_pos(bb.min - 2.0);
        let (end_cx, end_cz) = calc_entity_chunk_pos(bb.max + 2.0);

        EntitiesCollidingIter {
            chunks: ChunkComponentsIter { 
                chunks: &self.chunks, 
                range: ChunkRange::new(start_cx, start_cz, end_cx, end_cz) },
            indices: None,
            entities: &self.entities[..],
            bb,
        }

    }

    /// Iterate over all entities colliding with the given bounding box through mut ref.
    /// *This function can't return the current updated entity.*
    #[inline]
    pub fn iter_entities_colliding_mut(&mut self, bb: BoundingBox) -> EntitiesCollidingIterMut<'_> {
        
        let (start_cx, start_cz) = calc_entity_chunk_pos(bb.min - 2.0);
        let (end_cx, end_cz) = calc_entity_chunk_pos(bb.max + 2.0);

        EntitiesCollidingIterMut {
            chunks: ChunkComponentsIter { 
                chunks: &self.chunks, 
                range: ChunkRange::new(start_cx, start_cz, end_cx, end_cz) },
            indices: None,
            entities: &mut self.entities[..],
            bb,
            #[cfg(debug_assertions)]
            returned_pointers: HashSet::new(),
        }

    }

    // =================== //
    //       TICKING       //
    // =================== //
    
    /// Tick the world, this ticks all entities.
    /// TODO: Guard this from being called recursively from tick functions.
    #[instrument(skip_all)]
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
    #[instrument(skip_all)]
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
    #[instrument(skip_all)]
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
    #[instrument(skip_all)]
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
    #[instrument(skip_all)]
    fn tick_entities(&mut self) {

        let mut indices_to_remove = INDICES_TO_REMOVE.take();
        debug_assert!(indices_to_remove.is_empty());

        // NOTE: Only update the entities that are present at the start of ticking.
        for entity_index in 0..self.entities.len() {

            // We unwrap because all entities should be present except updated one.
            let entity_comp = &mut self.entities[entity_index];
            let mut entity = match entity_comp.inner {
                ComponentStorage::Removed => {
                    // The entity has been removed, we definitely remove it here.
                    indices_to_remove.push(entity_index);
                    continue;
                }
                ComponentStorage::Updated => panic!("entity was already being updated"),
                ComponentStorage::Ready(_) => {
                    // Do not update the entity if its chunk is not loaded.
                    if !entity_comp.loaded {
                        continue;
                    }
                    // If the entity is ready for update, set 'Updated' state.
                    match entity_comp.inner.replace(ComponentStorage::Updated) {
                        ComponentStorage::Ready(data) => data,
                        _ => unreachable!()
                    }
                }
            };

            let id = entity_comp.id;

            // Store the previous chunk, used when comparing with new chunk.
            let (prev_cx, prev_cz) = (entity_comp.cx, entity_comp.cz);
            entity.tick(&mut *self, id);

            // Entity is not dead so we re-insert its 
            let entity_comp = &mut self.entities[entity_index];
            entity_comp.loaded = true;
            match entity_comp.inner {
                ComponentStorage::Ready(_) => panic!("entity should not be ready"),
                ComponentStorage::Removed => {
                    // Entity has been removed while ticking.
                    indices_to_remove.push(entity_index);
                    continue;
                }
                ComponentStorage::Updated => {}
            }

            // Take all dirty flags.
            let pos_dirty = mem::take(&mut entity.0.pos_dirty);
            let look_dirty = mem::take(&mut entity.0.look_dirty);
            let vel_dirty = mem::take(&mut entity.0.vel_dirty);
            
            let new_chunk = pos_dirty.then_some(calc_entity_chunk_pos(entity.0.pos));

            if let Some(events) = &mut self.events {
                if pos_dirty {
                    events.push(Event::Entity { id, inner: EntityEvent::Position { pos: entity.0.pos } });
                }
                if look_dirty {
                    events.push(Event::Entity { id, inner: EntityEvent::Look { look: entity.0.look } });
                }
                if vel_dirty {
                    events.push(Event::Entity { id, inner: EntityEvent::Velocity { vel: entity.0.vel } });
                }
            }

            // Entity is still in updated state as expected.
            entity_comp.inner = ComponentStorage::Ready(entity);

            // Check if the entity moved to another chunk...
            if let Some((new_cx, new_cz)) = new_chunk {
                if (prev_cx, prev_cz) != (new_cx, new_cz) {

                    // NOTE: This part is really critical as this ensures Memory Safety
                    // in iterators and therefore avoids Undefined Behaviors. Each entity
                    // really needs to be in a single chunk at a time.
                    
                    let remove_success = self.chunks.get_mut(&(prev_cx, prev_cz))
                        .expect("entity previous chunk is missing")
                        .entities
                        .remove(&entity_index);
                    
                    debug_assert!(remove_success, "entity index not found in previous chunk");

                    // Update the world entity to its new chunk and orphan state.
                    entity_comp.cx = new_cx;
                    entity_comp.cz = new_cz;

                    // Insert the entity in its new chunk.
                    let new_chunk_comp = self.chunks.entry((new_cx, new_cz)).or_default();
                    new_chunk_comp.entities.insert(entity_index);
                    // Update the loaded flag of the entity depending on the new chunk
                    // being loaded or not.
                    entity_comp.loaded = new_chunk_comp.data.is_some();

                }
            }

        }

        // It's really important to understand that indices to remove have been pushed
        // from lower index to higher (because of iteration order). So we need to drain
        // in reverse in order to remove the last indices first, because of the swap we
        // are guaranteed that the swapped entity will not be in the indices to remove.
        for index_to_remove in indices_to_remove.drain(..).rev() {

            self.entities.swap_remove(index_to_remove);
            
            // Because we used swap remove, this may have moved the last entity (if
            // existing) to the removed entity index. We need to update its index in 
            // chunk or orphan entities.
            if let Some(swapped_comp) = self.entities.get(index_to_remove) {
                
                let entities = &mut self.chunks.get_mut(&(swapped_comp.cx, swapped_comp.cz))
                    .expect("entity was not in a chunk")
                    .entities;
            
                // The swapped entity was at the end, so the new length.
                let previous_index = self.entities.len();
                
                // Update the mapping from entity unique id to the new index.
                let previous_map_index = self.entities_id_map.insert(swapped_comp.id, index_to_remove);
                debug_assert_eq!(previous_map_index, Some(previous_index), "incoherent previous entity index");
            
                let remove_success = entities.remove(&previous_index);
                debug_assert!(remove_success, "entity index not found where it belongs");
                entities.insert(index_to_remove);
                
            }
            
        }

        INDICES_TO_REMOVE.set(indices_to_remove);

    }

    #[instrument(skip_all)]
    fn tick_block_entities(&mut self) {

        let mut indices_to_remove = INDICES_TO_REMOVE.take();
        debug_assert!(indices_to_remove.is_empty());

        // The logic is essentially the same has for entity ticking, but without handling
        // of position and update of chunks.
        for block_entity_index in 0..self.block_entities.len() {

            let block_entity_comp = &mut self.block_entities[block_entity_index];
            let mut block_entity = match block_entity_comp.inner {
                ComponentStorage::Removed => {
                    indices_to_remove.push(block_entity_index);
                    continue;
                }
                ComponentStorage::Updated => panic!("entity was already being updated"),
                ComponentStorage::Ready(_) => {
                    if !block_entity_comp.loaded {
                        continue;
                    }
                    match block_entity_comp.inner.replace(ComponentStorage::Updated) {
                        ComponentStorage::Ready(data) => data,
                        _ => unreachable!()
                    }
                }
            };

            // Tick the block entity at its position.
            let pos = block_entity_comp.pos;
            block_entity.tick(self, pos);

            // Re-insert the block entity in the world after update.
            let block_entity_comp = &mut self.block_entities[block_entity_index];
            match block_entity_comp.inner {
                ComponentStorage::Ready(_) => panic!("block entity should not be ready"),
                ComponentStorage::Removed => {
                    indices_to_remove.push(block_entity_index);
                }
                ComponentStorage::Updated => {
                    block_entity_comp.inner = ComponentStorage::Ready(block_entity);
                }
            }

        }

        for index_to_remove in indices_to_remove.drain(..).rev() {

            self.block_entities.swap_remove(index_to_remove);
            
            // Because we used swap remove, this may have moved the last entity (if
            // existing) to the removed entity index. We need to update its index in 
            // chunk or orphan entities.
            if let Some(swapped_comp) = self.block_entities.get(index_to_remove) {
                
                let (cx, cz) = calc_chunk_pos_unchecked(swapped_comp.pos);
                let block_entities = &mut self.chunks.get_mut(&(cx, cz))
                    .expect("block entity was not in a chunk")
                    .entities;
            
                // The swapped entity was at the end, so the new length.
                let previous_index = self.block_entities.len();
                
                // Update the mapping from entity unique id to the new index.
                let previous_map_index = self.block_entities_pos_map.insert(swapped_comp.pos, index_to_remove);
                debug_assert_eq!(previous_map_index, Some(previous_index), "incoherent previous block entity index");
            
                let remove_success = block_entities.remove(&previous_index);
                debug_assert!(remove_success, "entity index not found where it belongs");
                block_entities.insert(index_to_remove);
                
            }
            
        }

        INDICES_TO_REMOVE.set(indices_to_remove);

    }

    /// Tick pending light updates.
    #[instrument(skip_all)]
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
#[derive(Debug, Clone, PartialEq)]
pub enum Event {
    /// An event with a block.
    Block {
        /// The position of the block.
        pos: IVec3,
        /// Inner block event.
        inner: BlockEvent,
    },
    /// An event with an entity given its id.
    Entity {
        /// The unique id of the entity.
        id: u32,
        /// Inner entity event.
        inner: EntityEvent,
    },
    /// A block entity has been set at this position.
    BlockEntity {
        /// The block entity position.
        pos: IVec3,
        /// Inner block entity event.
        inner: BlockEntityEvent,
    },
    /// The weather in the world has changed.
    Weather {
        /// Previous weather in the world.
        prev: Weather,
        /// New weather in the world.
        new: Weather,
    },
    /// An event to debug and spawn block break particles at the given position.
    DebugParticle {
        /// The block position to spawn particles at.
        pos: IVec3,
        /// The block to break at this position.
        block: u8,
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum BlockEvent {
    /// A block has been changed in the world.
    Set {
        /// The new block id.
        id: u8,
        /// The new block metadata.
        metadata: u8,
        /// Previous block id.
        prev_id: u8,
        /// Previous block metadata.
        prev_metadata: u8,
    },
    /// Play the block activation sound at given position and id/metadata.
    Sound {
        /// Current id of the block.
        id: u8,
        /// Current metadata of the block.
        metadata: u8,
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum EntityEvent {
    /// The entity has been spawned.
    Spawn,
    /// The entity has been removed.
    Remove,
    /// The entity changed its position.
    Position {
        pos: DVec3,
    },
    /// The entity changed its look.
    Look {
        look: Vec2,
    },
    /// The entity changed its velocity.
    Velocity {
        vel: DVec3,
    },
    /// The entity has picked up another entity, such as arrow or item. Note that the
    /// target entity is not removed by this event, it's only a hint that this happened
    /// just before the entity may be removed.
    Pickup {
        /// The id of the picked up entity.
        target_id: u32,
    },
    /// The entity is damaged and the damage animation should be played by frontend.
    Damage,
    /// The entity is dead and the dead animation should be played by frontend.
    Dead,
    /// Update to the creeper data.
    Creeper {
        ignited: bool,
        powered: bool,
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum BlockEntityEvent {
    /// The block entity has been set at its position.
    Set,
    /// The block entity has been removed at its position.
    Remove,
    /// A block entity have seen some of its stored item stack changed.
    Storage {
        /// The storage targeted by this event.
        storage: BlockEntityStorage,
        /// The next item stack at this index.
        stack: ItemStack,
    },
    /// A block entity has made some progress.
    Progress {
        /// The kind of progress targeted by this event.
        progress: BlockEntityProgress,
        /// Progress value.
        value: u16,
    },
}

/// Represent the storage slot for a block entity.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum BlockEntityStorage {
    /// The storage slot is referencing a classic linear inventory at given index.
    Standard(u8),
    FurnaceInput,
    FurnaceOutput,
    FurnaceFuel,
}

/// Represent the progress update for a block entity.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum BlockEntityProgress {
    FurnaceSmeltTime,
    FurnaceBurnMaxTime,
    FurnaceBurnRemainingTime,
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
/// 
/// Note: cloning a chunk component will also clone the chunk's Arc, therefore the whole
/// chunk content is actually cloned only when written to.
#[derive(Default, Clone)]
struct ChunkComponent {
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
#[derive(Debug, Clone)]
struct EntityComponent {
    /// The entity storage.
    inner: ComponentStorage<Box<Entity>>,
    /// Unique entity id is duplicated here to allow us to access it event when entity
    /// is updating.
    id: u32,
    /// The chunk X coordinate where this component is cached.
    cx: i32,
    /// The chunk Z coordinate where this component is cached.
    cz: i32,
    /// True when the chunk this entity is in is loaded with data.
    loaded: bool,
}

/// Internal type for storing a world block entity.
#[derive(Debug, Clone)]
struct BlockEntityComponent {
    /// The block entity storage.
    inner: ComponentStorage<Box<BlockEntity>>,
    /// True when the chunk this block entity is in is loaded with data.
    loaded: bool,
    /// Position of that block entity.
    pos: IVec3,
}

/// State of a component storage.
#[derive(Debug, Clone)]
enum ComponentStorage<T> {
    /// The component is present and ready to update.
    Ready(T),
    /// The component is temporally owned by the tick function in order to update it.
    Updated,
    /// The component has been marked for removal and will be removed on new tick, the
    /// component should already be removed from the chunk component cache and from its
    /// world mapping ([entity id => index] or [block entity pos => index]).
    Removed,
}

impl<T> ComponentStorage<T> {

    /// If the inner storage is ready for update, return some shared reference to it.
    #[inline]
    fn as_ref(&self) -> Option<&T> {
        match self {
            Self::Ready(data) => Some(data),
            _ => None
        }
    }

    /// If the inner storage is ready for update, return some exclusive reference to it.
    #[inline]
    #[allow(unused)]
    fn as_mut(&mut self) -> Option<&mut T> {
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
#[derive(Clone, Eq)]
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
#[derive(Clone)]
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


/// An iterator for blocks in a world area. 
/// This yields the block position, id and metadata.
pub struct BlocksInIter<'a> {
    /// Back-reference to the containing world.
    world: &'a World,
    /// This contains a temporary reference to the chunk being analyzed. This is used to
    /// avoid repeatedly fetching chunks' map.
    chunk: Option<(i32, i32, Option<&'a Chunk>)>,
    /// Minimum coordinate to fetch.
    start: IVec3,
    /// Maximum coordinate to fetch (exclusive).
    end: IVec3,
    /// Next block to fetch.
    cursor: IVec3,
}

impl<'a> BlocksInIter<'a> {

    #[inline]
    fn new(world: &'a World, mut start: IVec3, mut end: IVec3) -> Self {

        debug_assert!(start.x <= end.x && start.y <= end.y && start.z <= end.z);

        start.y = start.y.clamp(0, CHUNK_HEIGHT as i32 - 1);
        end.y = end.y.clamp(0, CHUNK_HEIGHT as i32 - 1);

        // If one the component is in common, because max is exclusive, there will be no
        // blocks at all to read, so we set max to min so it will directly ends.
        if start.x == end.x || start.y == end.y || start.z == end.z {
            end = start;
        }

        Self {
            world,
            chunk: None,
            start,
            end,
            cursor: start,
        }

    }

}

impl FusedIterator for BlocksInIter<'_> {}
impl Iterator for BlocksInIter<'_> {

    type Item = (IVec3, u8, u8);

    fn next(&mut self) -> Option<Self::Item> {
        
        // X is the last updated component, so when it reaches max it's done.
        if self.cursor.x == self.end.x {
            return None;
        }

        // We are at the start of a new column, update the chunk.
        if self.cursor.y == self.start.y {
            // NOTE: Unchecked because the Y value is clamped in the constructor.
            let (cx, cz) = calc_chunk_pos_unchecked(self.cursor);
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

        // This component order is important because it matches the internal layout of
        // chunks, and therefore improve cache efficiency.
        self.cursor.y += 1;
        if self.cursor.y == self.end.y {
            self.cursor.y = self.start.y;
            self.cursor.z += 1;
            if self.cursor.z == self.end.z {
                self.cursor.z = self.start.z;
                self.cursor.x += 1;
            }
        }

        Some(ret)

    }

}


/// An iterator for blocks in a world chunk. 
pub struct BlocksInChunkIter<'a> {
    /// Back-reference to the containing world. None if the chunk doesn't exists or the
    /// iterator is exhausted.
    chunk: Option<&'a Chunk>,
    /// Current position that is iterated in the chunk.
    cursor: IVec3,
}

impl<'a> BlocksInChunkIter<'a> {

    #[inline]
    fn new(world: &'a World, cx: i32, cz: i32) -> Self {
        Self {
            chunk: world.get_chunk(cx, cz),
            cursor: IVec3::new(cx * CHUNK_WIDTH as i32, 0, cz * CHUNK_WIDTH as i32),
        }
    }

}

impl FusedIterator for BlocksInChunkIter<'_> {}
impl Iterator for BlocksInChunkIter<'_> {

    type Item = (IVec3, u8, u8);

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {

        let (block, metadata) = self.chunk?.get_block(self.cursor);
        let ret = (self.cursor, block, metadata);

        // This component order is important because it matches the internal layout of
        // chunks, and therefore improve cache efficiency. When incrementing component,
        // when we reach the next multiple of 16 (for X/Z), we reset the coordinate.
        self.cursor.y += 1;
        if self.cursor.y >= CHUNK_HEIGHT as i32 {
            self.cursor.y = 0;
            self.cursor.z += 1;
            if self.cursor.z & 0b1111 == 0 {
                self.cursor.z -= 16;
                self.cursor.x += 1;
                if self.cursor.x & 0b1111 == 0 {
                    // X is the last coordinate to be updated, when we reach it then we
                    // set chunk to none because iterator is exhausted.
                    self.chunk = None;
                }
            }
        }

        Some(ret)

    }

}

/// An iterator over all entities in the world.
pub struct EntitiesIter<'a>(slice::Iter<'a, EntityComponent>);

impl FusedIterator for EntitiesIter<'_> {}
impl ExactSizeIterator for EntitiesIter<'_> {}
impl<'a> Iterator for EntitiesIter<'a> {
    
    type Item = (u32, &'a Entity);

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        while let Some(comp) = self.0.next() {
            if let Some(ret) = comp.inner.as_deref() {
                return Some((comp.id, ret));
            }
        }
        None
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.0.size_hint()
    }

}

/// An iterator over all entities in the world through mutable references.
pub struct EntitiesIterMut<'a>(slice::IterMut<'a, EntityComponent>);

impl FusedIterator for EntitiesIterMut<'_> {}
impl ExactSizeIterator for EntitiesIterMut<'_> {}
impl<'a> Iterator for EntitiesIterMut<'a> {

    type Item = (u32, &'a mut Entity);

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        while let Some(comp) = self.0.next() {
            if let Some(ret) = comp.inner.as_deref_mut() {
                return Some((comp.id, ret));
            }
        }
        None
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.0.size_hint()
    }

}

/// An iterator of entities within a chunk.
pub struct EntitiesInChunkIter<'a> {
    /// The entities indices, returned indices are unique within the iterator.
    indices: Option<indexmap::set::Iter<'a, usize>>,
    /// The entities.
    entities: &'a [EntityComponent],
}

impl FusedIterator for EntitiesInChunkIter<'_> {}
impl<'a> Iterator for EntitiesInChunkIter<'a> {

    type Item = (u32, &'a Entity);

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        while let Some(&index) = self.indices.as_mut()?.next() {
            // We ignore updated entities.
            let comp = &self.entities[index];
            if let Some(entity) = comp.inner.as_deref() {
                return Some((comp.id, entity));
            }
        }
        None
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        if let Some(indices) = &self.indices {
            indices.size_hint()
        } else {
            (0, Some(0))
        }
    }

}

/// An iterator of entities within a chunk through mutable references.
pub struct EntitiesInChunkIterMut<'a> {
    /// The entities indices, returned indices are unique within the iterator.
    indices: Option<indexmap::set::Iter<'a, usize>>,
    /// The entities.
    entities: &'a mut [EntityComponent],
    /// Only used when debug assertions are enabled in order to ensure the safety
    /// of the lifetime transmutation.
    #[cfg(debug_assertions)]
    returned_pointers: HashSet<*mut Entity>,
}

impl FusedIterator for EntitiesInChunkIterMut<'_> {}
impl<'a> Iterator for EntitiesInChunkIterMut<'a> {

    type Item = (u32, &'a mut Entity);

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        while let Some(&index) = self.indices.as_mut()?.next() {
            // We ignore updated entities.
            let comp = &mut self.entities[index];
            if let Some(entity) = comp.inner.as_deref_mut() {

                // Only check uniqueness of returned pointer with debug assertions.
                #[cfg(debug_assertions)] {
                    assert!(self.returned_pointers.insert(entity), "wrong unsafe contract");
                }

                // SAFETY: We know that returned indices are unique because they come from
                // a map iterator that have unique "usize" keys. So each entity will be 
                // accessed and mutated once and in one place only. So we transmute the 
                // lifetime to 'a, instead of using the default `'self`. This is almost 
                // the same as the implementation of mutable slice iterators where we can
                // get mutable references to all slice elements at once.
                let entity = unsafe { &mut *(entity as *mut Entity) };
                return Some((comp.id, entity));

            }
        }
        None
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        if let Some(indices) = &self.indices {
            indices.size_hint()
        } else {
            (0, Some(0))
        }
    }

}

/// An iterator of entities within a chunk.
pub struct EntitiesCollidingIter<'a> {
    /// Chunk components iter whens indices is exhausted.
    chunks: ChunkComponentsIter<'a>,
    /// The entities indices, returned indices are unique within the iterator.
    indices: Option<indexmap::set::Iter<'a, usize>>,
    /// The entities.
    entities: &'a [EntityComponent],
    /// Bounding box to check.
    bb: BoundingBox,
}

impl FusedIterator for EntitiesCollidingIter<'_> {}
impl<'a> Iterator for EntitiesCollidingIter<'a> {

    type Item = (u32, &'a Entity);

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        // LOOP SAFETY: This loop should not cause infinite iterator because self.indices
        // will eventually be none because it is set to none when it is exhausted. 
        loop {

            if self.indices.is_none() {
                self.indices = Some(self.chunks.next()?.entities.iter());
            }

            // If there is no next index, set indices to none and loop over.
            if let Some(&index) = self.indices.as_mut().unwrap().next() {
                let comp = &self.entities[index];
                // We ignore updated/not colliding entities.
                if let Some(entity) = comp.inner.as_deref() {
                    if entity.0.bb.intersects(self.bb) {
                        return Some((comp.id, entity));
                    }
                }
            } else {
                self.indices = None;
            }

        }
    }

}

/// An iterator of entities within a chunk through mutable references.
pub struct EntitiesCollidingIterMut<'a> {
    /// Chunk components iter whens indices is exhausted.
    chunks: ChunkComponentsIter<'a>,
    /// The entities indices, returned indices are unique within the iterator.
    indices: Option<indexmap::set::Iter<'a, usize>>,
    /// The entities.
    entities: &'a mut [EntityComponent],
    /// Bounding box to check.
    bb: BoundingBox,
    /// Only used when debug assertions are enabled in order to ensure the safety
    /// of the lifetime transmutation.
    #[cfg(debug_assertions)]
    returned_pointers: HashSet<*mut Entity>,
}

impl FusedIterator for EntitiesCollidingIterMut<'_> {}
impl<'a> Iterator for EntitiesCollidingIterMut<'a> {

    type Item = (u32, &'a mut Entity);

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        // LOOP SAFETY: This loop should not cause infinite iterator because self.indices
        // will eventually be none because it is set to none when it is exhausted.
        loop {

            if self.indices.is_none() {
                self.indices = Some(self.chunks.next()?.entities.iter());
            }

            // If there is no next index, set indices to none and loop over.
            if let Some(&index) = self.indices.as_mut().unwrap().next() {
                let comp = &mut self.entities[index];
                // We ignore updated/not colliding entities.
                if let Some(entity) = comp.inner.as_deref_mut() {
                    if entity.0.bb.intersects(self.bb) {

                        #[cfg(debug_assertions)] {
                            assert!(self.returned_pointers.insert(entity), "wrong unsafe contract");
                        }

                        // SAFETY: Read safety note of 'EntitiesInChunkIterMut', however
                        // we have additional constraint, because we iterate different 
                        // index map iterators so we are no longer guaranteed uniqueness
                        // of returned indices. However, our world implementation ensures
                        // that any entity is only present in a single chunk.
                        let entity = unsafe { &mut *(entity as *mut Entity) };
                        return Some((comp.id, entity));
                        
                    }
                }
            } else {
                self.indices = None;
            }

        }
    }

}

/// Internal iterator chunk components in a range.
struct ChunkComponentsIter<'a> {
    /// Map of chunk components that we 
    chunks: &'a HashMap<(i32, i32), ChunkComponent>,
    /// The range of chunks to iterate on.
    range: ChunkRange,
}

impl FusedIterator for ChunkComponentsIter<'_> {}
impl<'a> Iterator for ChunkComponentsIter<'a> {

    type Item = &'a ChunkComponent;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        while let Some((cx, cz)) = self.range.next() {
            if let Some(comp) = self.chunks.get(&(cx, cz)) {
                return Some(comp);
            }
        }
        None
    }

}

/// Internal iterator of chunk coordinates, both start and end are inclusive.
struct ChunkRange {
    cx: i32,
    cz: i32,
    start_cx: i32,
    end_cx: i32,
    end_cz: i32,
}

impl ChunkRange {

    // Construct a chunk range iterator, note that both start and end are included in the
    // range.
    #[inline]
    fn new(start_cx: i32, start_cz: i32, end_cx: i32, end_cz: i32) -> Self {
        Self {
            cx: start_cx,
            cz: start_cz,
            start_cx,
            end_cx,
            end_cz,
        }
    }

}

impl FusedIterator for ChunkRange {}
impl Iterator for ChunkRange {

    type Item = (i32, i32);

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        
        if self.cx > self.end_cx || self.cz > self.end_cz {
            return None;
        }

        let ret = (self.cx, self.cz);

        self.cx += 1;
        if self.cx > self.end_cx {
            self.cx = self.start_cx;
            self.cz += 1;
        }

        Some(ret)

    }

}


#[cfg(test)]
mod tests {

    use super::*;

    #[test]
    fn chunk_range() {

        assert_eq!(ChunkRange::new(0, 0, 0, 0).collect::<Vec<_>>(), [(0, 0)]);
        assert_eq!(ChunkRange::new(0, 0, 1, 0).collect::<Vec<_>>(), [(0, 0), (1, 0)]);
        assert_eq!(ChunkRange::new(0, 0, 1, 1).collect::<Vec<_>>(), [(0, 0), (1, 0), (0, 1), (1, 1)]);
        assert_eq!(ChunkRange::new(0, 0, -1, 0).collect::<Vec<_>>(), []);
        assert_eq!(ChunkRange::new(0, 0, 0, -1).collect::<Vec<_>>(), []);
        assert_eq!(ChunkRange::new(0, 0, -1, -1).collect::<Vec<_>>(), []);

    }

}
