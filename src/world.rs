//! Data structure for storing a world (overworld or nether) at runtime.

use std::any::Any;
use std::collections::HashMap;
use std::iter::FusedIterator;
use std::ops::Add;

use glam::{IVec3, DVec3};

use crate::chunk::{Chunk, calc_chunk_pos};
use crate::entity::{self, EntityBehavior};
use crate::util::bb::BoundingBox;
use crate::block::block_from_id;


/// Calculate the chunk position corresponding to the given block position. 
/// This also returns chunk-local coordinates in this chunk.
#[inline]
pub fn calc_entity_chunk_pos(pos: DVec3) -> (i32, i32) {
    calc_chunk_pos(pos.as_ivec3())
}


/// Data structure for a whole world.
pub struct World {
    /// The dimension
    dimension: Dimension,
    /// The spawn position.
    spawn_pos: IVec3,
    /// The world time, increasing at each tick.
    time: u64,
    /// Pending events queue.
    events: Vec<Event>,
    /// Mapping of chunks to their coordinates.
    chunks: HashMap<(i32, i32), Box<Chunk>>,
    /// The entities are stored inside an option, this has no overhead because of niche 
    /// in the box type, but allows us to temporarily own the entity when updating it, 
    /// therefore avoiding borrowing issues.
    entities: Vec<Option<Box<dyn EntityGeneric>>>,
    /// Entities' index mapping from their unique id.
    entities_map: HashMap<u32, usize>,
    /// Next entity id apply to a newly spawned entity.
    next_entity_id: u32,
}

impl World {

    pub fn new(dimension: Dimension) -> Self {
        Self {
            dimension,
            spawn_pos: IVec3::ZERO,
            time: 0,
            events: Vec::new(),
            chunks: HashMap::new(),
            entities: Vec::new(),
            entities_map: HashMap::new(),
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
        self.chunks.get(&(cx, cz)).map(|c| &**c)
    }

    pub fn chunk_mut(&mut self, cx: i32, cz: i32) -> Option<&mut Chunk> {
        self.chunks.get_mut(&(cx, cz)).map(|c| &mut **c)
    }

    pub fn insert_chunk(&mut self, cx: i32, cz: i32, chunk: Box<Chunk>) {
        self.chunks.insert((cx, cz), chunk);
    }

    pub fn remove_chunk(&mut self, cx: i32, cz: i32) -> Option<Box<Chunk>> {
        self.chunks.remove(&(cx, cz))
    }

    /// Get block and metadata at given position in the world, if the chunk is not
    /// loaded, zeros are returned.
    pub fn block_and_metadata(&self, pos: IVec3) -> (u8, u8) {
        // FIXME: Check for y < 0 || y >= 128
        let (cx, cz) = calc_chunk_pos(pos);
        match self.chunk(cx, cz) {
            Some(chunk) => chunk.block_and_metadata(pos),
            None => (0, 0),
        }
    }

    pub fn set_block_and_metadata(&mut self, pos: IVec3, id: u8, metadata: u8) {
        // FIXME: Check for y < 0 || y >= 128
        let (cx, cz) = calc_chunk_pos(pos);
        let chunk = self.chunk_mut(cx, cz).unwrap();
        chunk.set_block_and_metadata(pos, id, metadata);
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
    fn add_entity(&mut self, id: u32, entity: Box<dyn EntityGeneric>) {
        let index = self.entities.len();
        self.entities.push(Some(entity));
        self.entities_map.insert(id, index);
        self.push_event(Event::EntitySpawn { id });
    }

    /// Spawn an entity in this world, this function.
    #[inline(always)]
    pub fn spawn_entity<I>(&mut self, entity: entity::Base<I>) -> u32
    where
        entity::Base<I>: EntityGeneric + Any,
    {
        let mut entity = Box::new(entity);
        let id = self.next_entity_id();
        entity.id = id;
        self.add_entity(id, entity);
        id
    }

    /// Get a generic entity from its unique id. This generic entity can later be checked
    /// for being of a particular type.
    #[track_caller]
    pub fn entity(&self, id: u32) -> Option<&dyn EntityGeneric> {
        let index = *self.entities_map.get(&id)?;
        Some(self.entities[index].as_deref().expect("entity is being updated"))
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
        Some(self.entities[index].as_deref_mut().expect("entity is being updated"))
    }

    /// Get an entity of a given type from its unique id. If an entity exists with this id
    /// but is not of the right type, none is returned.
    #[track_caller]
    pub fn entity_downcast_mut<E: EntityGeneric>(&mut self, id: u32) -> Option<&mut E> {
        self.entity_mut(id)?.downcast_mut()
    }

    /// Iterate over all blocks in the given area.
    /// Min is inclusive and max is exclusive.
    #[must_use]
    pub fn iter_area_blocks(&self, min: IVec3, max: IVec3) -> impl Iterator<Item = (IVec3, u8, u8)> + FusedIterator + '_ {
        WorldAreaBlocks {
            world: self,
            chunk: None,
            min,
            max,
            cursor: min,
        }
    }

    /// Iterate over all bounding boxes in the given area.
    /// Min is inclusive and max is exclusive.
    #[must_use]
    pub fn iter_area_bounding_boxes(&self, min: IVec3, max: IVec3) -> impl Iterator<Item = BoundingBox> + '_ {
        self.iter_area_blocks(min, max).flat_map(|(pos, id, metadata)| {
            let pos = pos.as_dvec3();
            block_from_id(id).bounding_boxes(metadata).iter().map(move |bb| bb.offset(pos))
        })
    }

    #[must_use]
    pub fn iter_colliding_bounding_boxes(&self, bb: BoundingBox) -> impl Iterator<Item = BoundingBox> + '_ {
        let min = bb.min.floor().as_ivec3();
        let max = bb.max.add(1.0).floor().as_ivec3();
        self.iter_area_bounding_boxes(min, max).filter(move |block_bb| block_bb.intersects(bb))
    }

    /// Tick the world, this ticks all entities.
    pub fn tick(&mut self) {

        self.time += 1;

        // For each entity, we take the box from its slot (moving 64 * 2 bits), therefore
        // taking the ownership, this allows us ticking it with the whole mutable world.
        for i in 0..self.entities.len() {
            
            // We unwrap because all entities should be present except updated one.
            let mut entity = self.entities[i].take().unwrap();
            entity.delegate_tick(&mut *self);
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


/// A trait used internally in the world to allow checking actual type of the entity.
pub trait EntityGeneric: EntityBehavior + Any {

    /// Get this entity as any type, this allows checking its real type.
    fn any(&self) -> &dyn Any;

    /// Get this entity as mutable any type.
    fn any_mut(&mut self) -> &mut dyn Any;

    /// Delegate to the real entity's tick.
    fn delegate_tick(&mut self, world: &mut World);

    /// Get the entity unique id.
    fn id(&self) -> u32;

    /// Get the position of the entity.
    fn pos(&self) -> DVec3;

}

impl dyn EntityGeneric {

    /// Check if this entity is of the given type.
    #[inline]
    pub fn is<E: EntityGeneric>(&self) -> bool {
        self.any().is::<E>()
    }

    #[inline]
    pub fn downcast_ref<E: EntityGeneric>(&self) -> Option<&E> {
        self.any().downcast_ref::<E>()
    }

    #[inline]
    pub fn downcast_mut<E: EntityGeneric>(&mut self) -> Option<&mut E> {
        self.any_mut().downcast_mut::<E>()
    }

}

impl<I> EntityGeneric for entity::Base<I>
where
    entity::Base<I>: EntityBehavior + Any,
{

    fn any(&self) -> &dyn Any {
        self
    }

    fn any_mut(&mut self) -> &mut dyn Any {
        self
    }
   
    fn delegate_tick(&mut self, world: &mut World) {
        EntityBehavior::tick(self, world)
    }

    fn id(&self) -> u32 {
        self.id
    }

    fn pos(&self) -> DVec3 {
        self.pos
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
            let (cx, cz) = calc_chunk_pos(cursor);
            if !matches!(self.chunk, Some((ccx, ccz, _)) if ccx == cx && ccz == cz) {
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
