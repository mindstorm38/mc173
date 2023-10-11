//! The driver is used to run tick updates on a world.

use crate::world::World;


/// The driver tracks a world and update it, it also provides events on what's happening
/// in the world.
pub struct Driver {
    /// Inner world being driven by this driver.
    world: World,
    /// The driver source for loading the world.
    source: Box<dyn Source>,
    /// Queue of events that happened since the last drain of events.
    world_events: Vec<Event>,
    /// The next entity id to allocate.
    next_entity_id: u32,
}

impl Driver {

    pub fn new(world: World, source: Box<dyn Source>) -> Self {
        Self {
            world,
            source,
            world_events: Vec::new(),
            next_entity_id: 0,
        }
    }

    pub fn world(&self) -> &World {
        &self.world
    }

    pub fn world_mut(&mut self) -> &mut World {
        &mut self.world
    }

    pub fn tick(&mut self) {
        
        

    }

}

/// Represent a source for loading world's chunks and entities. All of the functions are
/// called synchronously from the world's tick
pub trait Source {

    /// Tick this source, allowing it to modify the world, events should be added 
    /// accordingly to what is done to the world.
    fn tick(&mut self, world: &mut World, events: &mut Vec<Event>);

    /// Request a chunk to be loaded by this source (this can be asynchronous), when the
    /// source has loaded the chunk and it is ready, it should be added on the next call
    /// to the `tick` method.
    fn request_chunk(&mut self, cx: i32, cz: i32);

}

/// Enumeration of possible world events.
#[derive(Debug, Clone)]
pub enum Event {
    ChunkLoaded {
        cx: i32,
        cz: i32,
    },
    ChunkUnloaded {
        cx: i32,
        cz: i32,
    },
    EntitySpawned(u32),
    EntityKilled(u32),
}
