//! Data structure for storing a world (overworld or nether) at runtime.

use std::collections::HashMap;

use glam::{IVec3, DVec3};

use crate::chunk::{Chunk, calc_chunk_pos};
use crate::entity::Entity;


/// Calculate the chunk position corresponding to the given block position. 
/// This also returns chunk-local coordinates in this chunk.
#[inline]
pub fn calc_entity_chunk_pos(pos: DVec3) -> (i32, i32) {
    calc_chunk_pos(pos.x as i32, pos.z as i32)
}


/// Data structure for a whole world.
pub struct World {
    /// The dimension
    dimension: Dimension,
    /// The spawn position.
    spawn_pos: IVec3,
    /// The world time, increasing at each tick.
    time: u64,
    /// Mapping of chunks to their coordinates.
    chunks: HashMap<(i32, i32), Box<Chunk>>,
    /// The entities in this world, fast access to those entities is possible using their
    /// vector index instead of their unique id. This index is used to reference those
    /// entities in chunks. **When an entity die**, the entity is swap removed from this
    /// vector, therefore the entity that is swapped will see its index changed, it's
    /// therefore required to update the index of this entity in its chunk.
    entities: Vec<WorldEntity>,
    /// Retrieve an entity's index from its unique id.
    entities_map: HashMap<u32, usize>,
    /// List of entities that are not yet inside a chunk.
    orphan_entities: Vec<usize>,
}

impl World {

    pub fn new(dimension: Dimension) -> Self {
        Self {
            chunks: HashMap::new(),
            dimension,
            spawn_pos: IVec3::ZERO,
            time: 0,
            entities: Vec::new(),
            entities_map: HashMap::new(),
            orphan_entities: Vec::new(),
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

    pub fn chunk(&self, cx: i32, cz: i32) -> Option<&Chunk> {
        self.chunks.get(&(cx, cz)).map(|c| &**c)
    }

    pub fn chunk_mut(&mut self, cx: i32, cz: i32) -> Option<&mut Chunk> {
        self.chunks.get_mut(&(cx, cz)).map(|c| &mut **c)
    }

    pub fn insert_chunk(&mut self, cx: i32, cz: i32, chunk: Box<Chunk>) {
        self.chunks.insert((cx, cz), chunk);
        // TODO: Check for orphan entities.
    }

    pub fn remove_chunk(&mut self, cx: i32, cz: i32) -> Option<Box<Chunk>> {
        self.chunks.remove(&(cx, cz))
        // TODO: Unlink entities.
    }

    pub fn iter_entities(&self) -> impl Iterator<Item = &'_ Entity> {
        self.entities.iter().map(|e| &e.inner)
    }

    pub fn iter_entities_mut(&mut self) -> impl Iterator<Item = &'_ mut Entity> {
        self.entities.iter_mut().map(|e| &mut e.inner)
    }

    /// Add a new entity to this world, the only initial required values are its id, that
    /// should be unique across the server, and its position.
    pub fn new_entity(&mut self, id: u32, pos: DVec3) -> &mut Entity {

        let mut entity = Box::new(WorldEntity { 
            inner: Entity {
                id,
                pos,
                ..Default::default()
            }, 
            chunk_pos: None,
        });

        let entity_index = self.entities.len();

        let (cx, cz) = calc_entity_chunk_pos(pos);
        if let Some(chunk) = self.chunks.get_mut(&(cx, cz)) {
            chunk.add_entity(entity_index);
            entity.chunk_pos = Some((cx, cz));
        } else {
            self.orphan_entities.push(entity_index);
        }

        self.entities_map.insert(id, entity_index);
        self.entities.push(entity);

        &mut self.entities.last_mut().unwrap().inner

    }

    /// Remove an entity, given its unique id.
    pub fn remove_entity(&mut self, id: u32) -> bool {
        
        let Some(index) = self.entities_map.remove(&id) else { return false; };
        let entity = self.entities.swap_remove(index);
        let swapped_index = self.entities.len(); // The swapped 

        // Remove the entity from its chunk or orphan list.
        if let Some((cx, cz)) = entity.chunk_pos {
            self.chunks.get_mut(&(cx, cz))
                .expect("entity chunk incoherency")
                .remove_entity(index);
        } else {
            self.orphan_entities.remove(index);
        }
        
        // NOTE: We swapped the old entity with the last entity in the vector, therefore
        // its index changed, the last entity's index became the old entity's index.
        // We need to update chunks.

        if let Some(moved_entity) = self.entities.get_mut(index) {
            if let Some((cx, cz)) = moved_entity.chunk_pos {
                // If the entity is registered in a chunk, remove the index and re-add it.
                self.chunks.get_mut(&(cx, cz))
                    .expect("entity chunk incoherency")
                    .replace_entity(swapped_index, index);
            } else {
                // The moved entity is orphan, change its index.
                let position = self.orphan_entities.iter().position(|&idx| idx == swapped_index).unwrap();
                self.orphan_entities[position] = index;
            }
        }
        
        true

    }

    /// Update an entity's in this world, this actually checks if the entity has moved
    /// from its previous chunk, in such case the internal cache is updated.
    pub fn update_entity(&mut self, id: u32) -> bool {

        let Some(index) = self.entities_map.remove(&id) else { return false; };
        let entity = &self.entities[index];

        // TODO:

        true

    }

    pub fn entity(&self, id: u32) -> Option<&Entity> {
        self.entities_map.get(&id).map(|&index| &self.entities[index].inner)
    }

    pub fn entity_mut(&mut self, id: u32) -> Option<&mut Entity> {
        self.entities_map.get(&id).map(|&index| &mut self.entities[index].inner)
    }

    // /// Request a chunk to be loaded, return no chunk if the chunk is not available (and
    // /// have been requested if relevant), or return some chunk if the chunk is loaded.
    // pub fn request_chunk(&mut self, cx: i32, cz: i32) -> Option<&mut Chunk> {
    //     match self.chunks.entry((cx, cz)) {
    //         Entry::Occupied(o) => {
    //             match o.into_mut() {
    //                 ChunkState::Present(chunk) => Some(&mut **chunk),
    //                 ChunkState::Requested => None,
    //             }
    //         },
    //         Entry::Vacant(v) => {
    //             self.source.request_chunk(cx, cz);
    //             v.insert(ChunkState::Requested);
    //             None
    //         }
    //     }
    // }

    // fn tick(&mut self, events: &mut Vec<WorldEvent>) {

    //     // Poll every new available chunk.
    //     while let Some((cx, cz, chunk)) = self.source.poll_chunk() {
    //         if let Some(current_chunk) = self.chunks.get_mut(&(cx, cz)) {
    //             if let ChunkState::Requested = current_chunk {
    //                 *current_chunk = ChunkState::Present(chunk);
    //                 events.push(WorldEvent::ChunkLoaded { cx, cz });
    //             } else {
    //                 panic!("world source produced a chunk that was not requested");
    //             }
    //         } else {
    //             panic!("world source produced a chunk that is not existing");
    //         }
    //     }

    //     // Tick every entity.
    //     for entity in &mut self.entities {

    //         entity.lifetime += 1;

    //         match entity.kind {
    //             EntityKind::Item(item) => {

    //                 entity.vel.y -= 0.04;

    //             }
    //             _ => {}
    //         }

    //     }

    // }

}


/// Internal entity wrapper.
struct WorldEntity {
    /// Inner entity. 
    inner: Entity,
    /// The current chunk this entity is registered in.
    chunk_pos: Option<(i32, i32)>,
}


/// Types of dimensions, used for ambient effects in the world.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Dimension {
    /// The overworld dimension with a blue sky and day cycles.
    Overworld,
    /// The creepy nether dimension.
    Nether,
}
