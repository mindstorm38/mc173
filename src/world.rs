//! Data structure for storing a world (overworld or nether) at runtime.

use std::collections::HashMap;
use std::collections::hash_map::Entry;
use std::iter::FusedIterator;
use std::ops::{Add, Mul};

use glam::IVec3;

use crate::chunk::{Chunk, CHUNK_HEIGHT, calc_chunk_pos, calc_chunk_pos_unchecked, calc_entity_chunk_pos};
use crate::entity::{self, EntityLogic, ItemEntity, EntityGeneric};
use crate::util::rand::JavaRandom;
use crate::util::bb::BoundingBox;
use crate::block::block_from_id;


/// Data structure for a whole world.
pub struct World {
    /// The dimension
    dimension: Dimension,
    /// The spawn position.
    spawn_pos: IVec3,
    /// The world time, increasing at each tick.
    time: u64,
    /// The world's global random number generator.
    rand: JavaRandom,
    /// Pending events queue.
    events: Vec<Event>,
    /// Mapping of chunks to their coordinates.
    chunks: HashMap<(i32, i32), WorldChunk>,
    /// The entities are stored inside an option, this has no overhead because of niche 
    /// in the box type, but allows us to temporarily own the entity when updating it, 
    /// therefore avoiding borrowing issues.
    entities: Vec<Option<Box<dyn WorldEntityGeneric>>>,
    /// Entities' index mapping from their unique id.
    entities_map: HashMap<u32, usize>,
    /// List of entities that are not belonging to any chunk at the moment.
    orphan_entities: Vec<usize>,
    /// Next entity id apply to a newly spawned entity.
    next_entity_id: u32,
}

impl World {

    pub fn new(dimension: Dimension) -> Self {
        Self {
            dimension,
            spawn_pos: IVec3::ZERO,
            time: 0,
            rand: JavaRandom::new_seeded(),
            events: Vec::new(),
            chunks: HashMap::new(),
            entities: Vec::new(),
            entities_map: HashMap::new(),
            orphan_entities: Vec::new(),
            next_entity_id: 0,
        }
    }

    pub fn dimension(&self) -> Dimension {
        self.dimension
    }

    pub fn spawn_pos(&self) -> IVec3 {
        self.spawn_pos
    }

    pub fn set_spawn_pos(&mut self, pos: IVec3) {
        self.spawn_pos = pos;
    }

    pub fn time(&self) -> u64 {
        self.time
    }

    pub fn set_time(&mut self, time: u64) {
        self.time = time;
    }

    /// Push an event in this world.
    pub fn push_event(&mut self, event: Event) {
        self.events.push(event);
    }

    pub fn chunk(&self, cx: i32, cz: i32) -> Option<&Chunk> {
        self.chunks.get(&(cx, cz)).map(|c| &*c.inner)
    }

    pub fn chunk_mut(&mut self, cx: i32, cz: i32) -> Option<&mut Chunk> {
        self.chunks.get_mut(&(cx, cz)).map(|c| &mut *c.inner)
    }

    /// Insert a new chunk into the world. This function also generate an event for a new
    /// chunk and also take cares of internal coherency with potential orphan entities 
    /// that are placed in this new chunk.
    pub fn insert_chunk(&mut self, cx: i32, cz: i32, chunk: Box<Chunk>) {
        match self.chunks.entry((cx, cz)) {
            // There was no chunk here, so we check in orphan entities if there are
            // entities currently in this chunk's position.
            Entry::Vacant(v) => {

                let mut world_chunk = WorldChunk {
                    inner: chunk,
                    entities: Vec::new(),
                };

                self.orphan_entities.retain(|&entity_index| {

                    let entity = self.entities[entity_index]
                        .as_deref_mut()
                        .expect("insert chunk while updating entity");
                    
                    // If the entity is in the newly added chunk.
                    let entity_chunk = entity.chunk_mut();
                    if entity_chunk.cx == cx && entity_chunk.cz == cz {
                        world_chunk.entities.push(entity_index);
                        entity_chunk.orphan = false;
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
                    entities: Vec::new(),
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
            self.entities[entity_index]
                .as_deref_mut()
                .expect("remove chunk while updating entity")
                .chunk_mut()
                .orphan = true;
        }

        self.orphan_entities.extend_from_slice(&chunk.entities);
        Some(chunk.inner)

    }

    /// Get block and metadata at given position in the world, if the chunk is not
    /// loaded, none is returned.
    pub fn block_and_metadata(&self, pos: IVec3) -> Option<(u8, u8)> {
        let (cx, cz) = calc_chunk_pos(pos)?;
        let chunk = self.chunk(cx, cz)?;
        Some(chunk.block_and_metadata(pos))
    }

    /// Internal function to ensure monomorphization and reduce bloat of the 
    /// generic [`spawn_entity`].
    #[inline(never)]
    fn next_entity_id(&mut self) -> u32 {
        let id = self.next_entity_id;
        self.next_entity_id = self.next_entity_id.checked_add(1)
            .expect("entity id overflow");
        id
    }

    /// Internal function to ensure monomorphization and reduce bloat of the 
    /// generic [`spawn_entity`].
    #[inline(never)]
    fn add_entity(&mut self, id: u32, mut entity: Box<dyn WorldEntityGeneric>) {

        let entity_index = self.entities.len();

        // Bind the entity to an existing chunk if possible.
        let (cx, cz) = calc_entity_chunk_pos(entity.entity().pos());
        let entity_chunk = entity.chunk_mut();
        entity_chunk.cx = cx;
        entity_chunk.cz = cz;
        
        if let Some(chunk) = self.chunks.get_mut(&(cx, cz)) {
            chunk.entities.push(entity_index);
            entity_chunk.orphan = false;
        } else {
            self.orphan_entities.push(entity_index);
            entity_chunk.orphan = true;
        }

        self.entities.push(Some(entity));
        self.entities_map.insert(id, entity_index);

        self.push_event(Event::EntitySpawn { id });

    }

    /// Spawn an entity in this world, this function gives it a unique id and ensure 
    /// coherency with chunks cache.
    #[inline]
    pub fn spawn_entity<B>(&mut self, entity: entity::Base<B>) -> u32
    where
        entity::Base<B>: EntityLogic
    {
        
        let mut entity = Box::new(WorldEntity {
            inner: entity,
            chunk: WorldEntityChunk::default(),
        });

        let id = self.next_entity_id();
        entity.inner.id = id;
        self.add_entity(id, entity);
        id

    }

    /// Kill an entity given its id, this function ensure coherency with chunks cache.
    pub fn kill_entity(&mut self, id: u32) -> bool {

        let Some(entity_index) = self.entities_map.remove(&id) else { return false };
        self.entities.swap_remove(entity_index).expect("kill an updating entity");

        // Because we used swap remove, this may have moved the last entity (if
        // existing) to the old entity index. We need to update its index in chunk
        // or orphan entities.
        if let Some(entity) = self.entities.get(entity_index) {
            // Check if the swapped entity is not updating, if not, get its chunk info.
            let entity_chunk = entity.as_ref()
                .expect("swap updating entity")
                .chunk();
            // Get the correct entities list depending on the entity being orphan or not.
            let chunk_entities = if entity_chunk.orphan {
                &mut self.orphan_entities[..]
            } else {
                &mut self.chunks.get_mut(&(entity_chunk.cx, entity_chunk.cz))
                    .expect("non-orphan entity referencing a non-existing chunk")
                    .entities[..]
            };
            // The swapped entity was at the end, so the new length.
            let previous_index = self.entities.len();
            // Find where the index is located in the array and modify it.
            let entity_index_index = chunk_entities.iter().position(|&index| index == previous_index)
                .expect("entity index not found where it belongs");
            chunk_entities[entity_index_index] = entity_index;
        }

        true

    }

    /// Get a generic entity from its unique id. This generic entity can later be checked
    /// for being of a particular type.
    #[track_caller]
    pub fn entity(&self, id: u32) -> Option<&dyn EntityGeneric> {
        let index = *self.entities_map.get(&id)?;
        Some(self.entities[index].as_deref()
            .map(WorldEntityGeneric::entity)
            .expect("entity is being updated"))
    }

    /// Get an entity of a given type from its unique id. If an entity exists with this id
    /// but is not of the right type, none is returned.
    #[track_caller]
    pub fn entity_downcast<E: EntityGeneric>(&self, id: u32) -> Option<&E> {
        self.entity(id)?.downcast_ref()
    }

    /// Get a generic entity from its unique id. This generic entity can later be checked
    /// for being of a particular type.
    #[track_caller]
    pub fn entity_mut(&mut self, id: u32) -> Option<&mut dyn EntityGeneric> {
        let index = *self.entities_map.get(&id)?;
        Some(self.entities[index].as_deref_mut()
            .map(WorldEntityGeneric::entity_mut)
            .expect("entity is being updated"))
    }

    /// Get an entity of a given type from its unique id. If an entity exists with this id
    /// but is not of the right type, none is returned.
    #[track_caller]
    pub fn entity_downcast_mut<E: EntityGeneric>(&mut self, id: u32) -> Option<&mut E> {
        self.entity_mut(id)?.downcast_mut()
    }

    /// Iterate over all blocks in the given area.
    /// *Min is inclusive and max is exclusive.*
    pub fn iter_area_blocks(&self, min: IVec3, max: IVec3) -> impl Iterator<Item = (IVec3, u8, u8)> + FusedIterator + '_ {
        WorldAreaBlocks::new(self, min, max)
    }

    /// Iterate over all bounding boxes in the given area.
    /// *Min is inclusive and max is exclusive.*
    pub fn iter_area_bounding_boxes(&self, min: IVec3, max: IVec3) -> impl Iterator<Item = BoundingBox> + '_ {
        self.iter_area_blocks(min, max).flat_map(|(pos, id, metadata)| {
            let pos = pos.as_dvec3();
            block_from_id(id).bounding_boxes(metadata).iter().map(move |bb| bb.offset(pos))
        })
    }

    /// Iterate over all bounding boxes in the given area that are colliding with the 
    /// given one. *Min is inclusive and max is exclusive.*
    pub fn iter_colliding_bounding_boxes(&self, bb: BoundingBox) -> impl Iterator<Item = BoundingBox> + '_ {
        let min = bb.min.floor().as_ivec3();
        let max = bb.max.add(1.0).floor().as_ivec3();
        self.iter_area_bounding_boxes(min, max).filter(move |block_bb| block_bb.intersects(bb))
    }

    /// Iterate over all entities of the given chunk. This is legal for non-existing 
    /// chunks, in such case this will search for orphan entities.
    pub fn iter_chunk_entities(&self, cx: i32, cz: i32) -> impl Iterator<Item = &dyn EntityGeneric> {
        
        let (entities, orphan) = self.chunks.get(&(cx, cz))
            .map(|c| (&c.entities[..], false))
            .unwrap_or((&self.orphan_entities, true));

        entities.iter()
            .filter_map(move |&entity_index| {
                let entity = self.entities[entity_index].as_deref()?;
                // If we are iterating the orphan entities, check the chunk.
                if orphan {
                    let entity_chunk = entity.chunk();
                    if (entity_chunk.cx, entity_chunk.cz) != (cx, cz) {
                        return None;
                    }
                }
                Some(entity.entity())
            })

    }

    /// Break a block naturally and drop its items. This function will generate an event 
    /// of the block break and the items spawn. This returns true if successful, false
    /// if the chunk/pos was not valid.
    pub fn break_block(&mut self, pos: IVec3) -> bool {

        let Some((cx, cz)) = calc_chunk_pos(pos) else { return false };
        let Some(chunk) = self.chunk_mut(cx, cz) else { return false };

        let (prev_block, prev_metadata) = chunk.block_and_metadata(pos);
        chunk.set_block_and_metadata(pos, 0, 0);

        self.push_event(Event::BlockChange { 
            pos,
            prev_block, 
            prev_metadata, 
            new_block: 0, 
            new_metadata: 0,
        });

        const SPREAD: f32 = 0.7;
        let delta = self.rand.next_vec3()
            .mul(SPREAD)
            .as_dvec3()
            .add((1.0 - SPREAD as f64) * 0.5);

        let mut entity = ItemEntity::new(pos.as_dvec3() + delta);
        entity.vel.x = self.rand.next_double() * 0.2 - 0.1;
        entity.vel.y = 0.2;
        entity.vel.z = self.rand.next_double() * 0.2 - 0.1;
        entity.base.item.id = prev_block as u16;
        entity.base.item.size = 1;
        entity.base.time_before_pickup = 10;
        
        self.spawn_entity(entity);

        true
        
    }

    /// Tick the world, this ticks all entities.
    pub fn tick(&mut self) {

        self.time += 1;

        // For each entity, we take the box from its slot (moving 64 * 2 bits), therefore
        // taking the ownership, this allows us ticking it with the whole mutable world.
        for i in 0..self.entities.len() {
            
            // We unwrap because all entities should be present except updated one.
            let mut entity = self.entities[i].take().unwrap();
            entity.tick(&mut *self);
            // After tick, we re-add the entity.
            self.entities[i] = Some(entity);

        }

    }

    /// Iterate and remove all events.
    pub fn drain_events(&mut self) -> impl Iterator<Item = Event> + '_ {
        self.events.drain(..)
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Event {
    /// A new entity has been spawned.
    EntitySpawn {
        /// The unique id of the spawned entity.
        id: u32,
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
}


/// Internal type for storing a world chunk with its cached entities.
struct WorldChunk {
    /// Underlying chunk.
    inner: Box<Chunk>,
    /// Entities belonging to this chunk.
    entities: Vec<usize>,
}

/// Internal type for storing a world entity and keep track of its current chunk.
struct WorldEntity<I> {
    /// Underlying entity.
    inner: entity::Base<I>,
    /// Information about the chunk the entity is in when it was last updated.
    chunk: WorldEntityChunk,
}

#[derive(Default)]
struct WorldEntityChunk {
    /// The last computed chunk position X.
    cx: i32,
    /// The last computed chunk position Z.
    cz: i32,
    /// Indicate if this entity is orphan and therefore does not belong to any chunk.
    orphan: bool,
}

trait WorldEntityGeneric {

    /// Delegate tick to actual entity.
    fn tick(&mut self, world: &mut World);

    /// Return the underlying generic entity. 
    fn entity(&self) -> &dyn EntityGeneric;

    /// Return the underlying generic entity as mutable.
    fn entity_mut(&mut self) -> &mut dyn EntityGeneric;

    /// Get access to the underlying entity chunk information.
    fn chunk(&self) -> &WorldEntityChunk;

    /// Get mutable access to the underlying entity chunk information.
    fn chunk_mut(&mut self) -> &mut WorldEntityChunk;

}

impl<I> WorldEntityGeneric for WorldEntity<I>
where
    entity::Base<I>: EntityLogic 
{
    
    fn tick(&mut self, world: &mut World) {
        self.inner.tick(world);
    }

    fn entity(&self) -> &dyn EntityGeneric {
        &self.inner
    }

    fn entity_mut(&mut self) -> &mut dyn EntityGeneric {
        &mut self.inner
    }

    fn chunk(&self) -> &WorldEntityChunk {
        &self.chunk
    }

    fn chunk_mut(&mut self) -> &mut WorldEntityChunk {
        &mut self.chunk
    }

}


/// An iterator for blocks in a world area. This returns the block id and metadata.
struct WorldAreaBlocks<'a> {
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

impl<'a> WorldAreaBlocks<'a> {

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

        WorldAreaBlocks {
            world,
            chunk: None,
            min,
            max,
            cursor: min,
        }

    }

}

impl<'a> FusedIterator for WorldAreaBlocks<'a> {}
impl<'a> Iterator for WorldAreaBlocks<'a> {

    type Item = (IVec3, u8, u8);

    fn next(&mut self) -> Option<Self::Item> {
        
        let cursor = self.cursor;

        if cursor == self.max {
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
