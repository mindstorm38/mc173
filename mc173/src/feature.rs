//! Abstract world access trait and features.

use std::ops::Add;

use glam::{DVec3, IVec3, Vec2};

use crate::block_entity::BlockEntity;
use crate::geom::{BoundingBox, Face};
use crate::rand::JavaRandom;
use crate::item::ItemStack;
use crate::entity::Entity;
use crate::biome::Biome;


/// Abstract trait to access a single dimension.
/// 
/// For now, this trait is not object-safe.
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
pub trait Dimension {

    // =================== //
    //         MISC        //
    // =================== //

    /// Get the dimension of this world, this is basically only for sky color on client
    /// and also for celestial angle on the server side for sky light calculation. This
    /// has not direct relation with the actual world generation that is providing this
    /// world with chunks and entities.
    fn get_kind(&self) -> DimensionKind;

    /// Get the world time, in ticks.
    fn get_time(&self) -> u64;

    /// Get a mutable reference to the interval Java RNG.
    fn get_rand_mut(&mut self) -> &mut JavaRandom;

    // =================== //
    //        EVENTS       //
    // =================== //

    fn push_event(&mut self, event: Event);

    // =================== //
    //        BLOCKS       //
    // =================== //

    /// Set block and metadata at given position in the world, if the chunk is not
    /// loaded, none is returned, but if it is existing the previous block and metadata
    /// is returned. This function also push a block change event and update lights
    /// accordingly.
    fn set_block(&mut self, pos: IVec3, id: u8, metadata: u8) -> Option<(u8, u8)>;
    
    /// Get block and metadata at given position in the world, if the chunk is not
    /// loaded, none is returned.
    fn get_block(&self, pos: IVec3) -> Option<(u8, u8)>;

    // =================== //
    //        HEIGHT       //
    // =================== //

    /// Get saved height of a chunk column, Y component is ignored in the position. The
    /// returned height is a signed 32 bit integer, but the possible value is only in 
    /// range 0..=128, but it's easier to deal with `i32` because of vectors.
    fn get_height(&self, pos: IVec3) -> Option<i32>;

    // =================== //
    //        LIGHTS       //
    // =================== //

    /// Get light level at the given position, in range 0..16.
    fn get_light(&self, pos: IVec3) -> Light;

    // =================== //
    //        BIOMES       //
    // =================== //

    /// Get the biome at some position (Y component is ignored).
    fn get_biome(&self, pos: IVec3) -> Option<Biome>;

    // =================== //
    //       WEATHER       //
    // =================== //

    /// Get the current weather in the world.
    fn get_weather(&self) -> Weather;

    /// Set the current weather in this world. If the weather has changed an event will
    /// be pushed into the events queue.
    fn set_weather(&mut self, weather: Weather);

    /// Return the local weather at a given position, this may be different from global
    /// weather depending on local biome and local height.
    fn get_local_weather(&mut self, pos: IVec3) -> LocalWeather;

    // =================== //
    //       ENTITIES      //
    // =================== //

    /// Spawn an entity in this world, this function gives it a unique id and ensure 
    /// coherency with chunks cache.
    /// 
    /// **This function is legal to call from ticking entities, but such entities will be
    /// ticked once in the same cycle as the currently ticking entity.**
    fn spawn_entity(&mut self, entity: impl Into<Box<Entity>>) -> u32
    where Self: Sized;

    /// Remove an entity with given id, returning some boxed entity is successful. This
    /// returns true if the entity has been successfully removed removal, the entity's
    /// storage is guaranteed to be freed after return, but the entity footprint in the
    /// world will be cleaned only after ticking.
    fn remove_entity(&mut self, id: u32, reason: &str) -> bool;

    /// Return the number of entities in the world, loaded or not.
    fn count_entity(&self) -> usize;

    /// Return true if an entity is present from its id.
    fn has_entity(&self, id: u32) -> bool;

    /// Get a generic entity from its unique id. This generic entity can later be checked
    /// for being of a particular type. None can be returned if no entity is existing for
    /// this id or if the entity is the current entity being updated.
    fn get_entity(&self, id: u32) -> Option<&Entity>;

    /// Get a generic entity from its unique id. This generic entity can later be checked
    /// for being of a particular type. None can be returned if no entity is existing for
    /// this id or if the entity is the current entity being updated.
    fn get_entity_mut(&mut self, id: u32) -> Option<&mut Entity>;

    // =================== //
    //   PLAYER ENTITIES   //
    // =================== //

    /// Set an entity that is already existing to be a player entity. Player entities are
    /// used as dynamic anchors in the world that are used for things like natural entity
    /// despawning when players are too far away, or for looking at players.
    /// 
    /// This methods returns true if the property has been successfully set.
    fn set_player_entity(&mut self, id: u32, player: bool) -> bool;

    /// Returns the number of player entities in the world, loaded or not.
    fn count_player_entity(&self) -> usize;

    /// Returns true if the given entity by its id is a player entity. This also returns
    /// false if the entity isn't existing.
    fn is_player_entity(&mut self, id: u32) -> bool;

    // =================== //
    //   BLOCK ENTITIES    //
    // =================== //

    /// Set the block entity at the given position. If a block entity was already at the
    /// position, it is removed silently.
    fn set_block_entity(&mut self, pos: IVec3, block_entity: impl Into<Box<BlockEntity>>)
    where Self: Sized;

    /// Remove a block entity from a position. Returning true if successful, in this case
    /// the block entity storage is guaranteed to be freed, but the block entity footprint
    /// in this world will be definitely cleaned after ticking.
    fn remove_block_entity(&mut self, pos: IVec3) -> bool;

    /// Return the number of block entities in the world, loaded or not.
    fn count_block_entity(&self) -> usize;

    /// Returns true if some block entity is present in the world.
    fn has_block_entity(&self, pos: IVec3) -> bool;

    /// Get a block entity from its position.
    fn get_block_entity(&self, pos: IVec3) -> Option<&BlockEntity>;

    /// Get a block entity from its position.
    fn get_block_entity_mut(&mut self, pos: IVec3) -> Option<&mut BlockEntity>;
    
    // =================== //
    //   SCHEDULED TICKS   //
    // =================== //

    /// Schedule a tick update to happen at the given position, for the given block id
    /// and with a given delay in ticks. The block tick is not scheduled if a tick was
    /// already scheduled for that exact block id and position.
    fn schedule_block_tick(&mut self, pos: IVec3, id: u8, delay: u64);

    /// Return the current number of scheduled block ticks waiting.
    fn count_block_tick(&self) -> usize;

    // =================== //
    //      ITERATORS      //
    // =================== //

    /// Iterate over all blocks in the given area where max is excluded. Unloaded chunks
    /// are not yielded, so the iterator size cannot be known only from min and max.
    fn iter_blocks_in(&self, min: IVec3, max: IVec3) -> impl Iterator<Item = (IVec3, u8, u8)> + '_
    where Self: Sized;

    /// Iterate over all blocks in the chunk at given coordinates.
    fn iter_blocks_in_chunk(&self, cx: i32, cz: i32) -> impl Iterator<Item = (IVec3, u8, u8)> + '_
    where Self: Sized;

    /// Iterate over all block entities in a chunk.
    fn iter_block_entities_in_chunk(&self, cx: i32, cz: i32) -> impl Iterator<Item = (IVec3, &'_ BlockEntity)> + '_
    where Self: Sized;

    /// Iterate over all block entities in a chunk through mutable references.
    fn iter_block_entities_in_chunk_mut(&mut self, cx: i32, cz: i32) -> impl Iterator<Item = (IVec3, &'_ mut BlockEntity)> + '_
    where Self: Sized;

    /// Iterate over all entities in the world.
    /// *This function can't return the current updated entity.*
    fn iter_entities(&self) -> impl Iterator<Item = (u32, &'_ Entity)> + '_
    where Self: Sized;

    /// Iterator over all entities in the world through mutable references.
    /// *This function can't return the current updated entity.*
    fn iter_entities_mut(&mut self) -> impl Iterator<Item = (u32, &'_ mut Entity)> + '_
    where Self: Sized;

    /// Iterate over all player entities in the world.
    /// *This function can't return the current updated entity.*
    fn iter_player_entities(&self) -> impl Iterator<Item = (u32, &'_ Entity)> + '_
    where Self: Sized;

    /// Iterate over all player entities in the world through mutable references.
    /// *This function can't return the current updated entity.*
    fn iter_player_entities_mut(&mut self) -> impl Iterator<Item = (u32, &'_ mut Entity)> + '_
    where Self: Sized;

    /// Iterate over all entities of the given chunk.
    /// *This function can't return the current updated entity.*
    fn iter_entities_in_chunk(&self, cx: i32, cz: i32) -> impl Iterator<Item = (u32, &'_ Entity)> + '_
    where Self: Sized;

    /// Iterate over all entities of the given chunk through mutable references.
    /// *This function can't return the current updated entity.*
    fn iter_entities_in_chunk_mut(&mut self, cx: i32, cz: i32) -> impl Iterator<Item = (u32, &'_ mut Entity)> + '_
    where Self: Sized;

    /// Iterate over all entities colliding with the given bounding box.
    /// *This function can't return the current updated entity.*
    fn iter_entities_colliding(&self, bb: BoundingBox) -> impl Iterator<Item = (u32, &'_ Entity)> + '_
    where Self: Sized;

    /// Iterate over all entities colliding with the given bounding box through mut ref.
    /// *This function can't return the current updated entity.*
    fn iter_entities_colliding_mut(&mut self, bb: BoundingBox) -> impl Iterator<Item = (u32, &'_ mut Entity)> + '_
    where Self: Sized;
    
    /// Return true if any entity is colliding the given bounding box. The hard argument
    /// can be set to true in order to only check for "hard" entities, hard entities can
    /// prevent block placements and entity spawning.
    fn has_entity_colliding(&self, bb: BoundingBox, hard: bool) -> bool
    where Self: Sized {
        self.iter_entities_colliding(bb)
            .any(|(_, entity)| !hard || entity.kind().is_hard())
    }

}


/// Types of dimensions, used for ambient effects in the world.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DimensionKind {
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

/// Type of weather at a specific position.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LocalWeather {
    /// The weather is clear at the position.
    Clear,
    /// It is raining at the position.
    Rain,
    /// It is snowing at the position.
    Snow,
}

/// Light values of a position in the world.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Light {
    /// Block light level.
    pub block: u8,
    /// Sky light level.
    pub sky: u8,
    /// The real sky light level, depending on the time and weather.
    pub sky_real: u8,
}

impl Light {

    /// Calculate the maximum static light level (without time/weather attenuation).
    #[inline]
    pub fn max(self) -> u8 {
        u8::max(self.block, self.sky)
    }

    /// Calculate the maximum real light level (with time/weather attenuation).
    #[inline]
    pub fn max_real(self) -> u8 {
        u8::max(self.block, self.sky_real)
    }

    /// Calculate the block brightness from its light levels.
    #[inline]
    pub fn brightness(self) -> f32 {
        // TODO: In nether, OFFSET is 0.1
        const OFFSET: f32 = 0.05;
        let base = 1.0 - self.max_real() as f32 / 15.0;
        (1.0 - base) * (base * 3.0 + 1.0) * (1.0 - OFFSET) + OFFSET
    }

}

/// Different kind of lights in the word.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum LightKind {
    /// Block light level, the light spread in all directions and blocks have a minimum 
    /// opacity of 1 in all directions, each block has its own light emission.
    Block,
    /// Sky light level, same as block light but light do not decrease when going down
    /// and every block above height have is has an emission of 15.
    Sky,
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
    /// A chunk event.
    Chunk {
        /// The chunk X position.
        cx: i32,
        /// The chunk Z position.
        cz: i32,
        /// Inner chunk event.
        inner: ChunkEvent,
    },
    /// The weather in the world has changed.
    Weather {
        /// Previous weather in the world.
        prev: Weather,
        /// New weather in the world.
        new: Weather,
    },
    /// Explode blocks.
    Explode {
        /// Center position of the explosion.
        center: DVec3,
        /// Radius of the explosion around center.
        radius: f32,
    },
    /// An event to debug and spawn block break particles at the given position.
    DebugParticle {
        /// The block position to spawn particles at.
        pos: IVec3,
        /// The block to break at this position.
        block: u8,
    }
}

/// An event with a block.
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
    },
    /// A piston has been extended or retracted at the given position.
    Piston {
        /// Face of this piston.
        face: Face,
        /// True if the piston is extending.
        extending: bool,
    },
    /// A note block is playing its note.
    NoteBlock {
        /// The instrument to play.
        instrument: u8,
        /// The note to play.
        note: u8,
    },
}

/// An event with an entity.
#[derive(Debug, Clone, PartialEq)]
pub enum EntityEvent {
    /// The entity has been spawned. The initial chunk position is given.
    Spawn,
    /// The entity has been removed. The last chunk position is given.
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
    /// Some unspecified entity metadata has changed.
    Metadata,
}

/// An event with a block entity.
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
    /// A sign block entity has been modified.
    Sign,
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

/// An event with a chunk.
#[derive(Debug, Clone, PartialEq)]
pub enum ChunkEvent {
    /// The chunk has been set at its position. A chunk may have been replaced at that
    /// position.
    Set,
    /// The chunk has been removed from its position.
    Remove,
    /// Any chunk component (block, light, entity, block entity) has been modified in the
    /// chunk so it's marked dirty.
    Dirty,
}


/// Trait extension to world that provides various block boxes and ray tracing.
pub trait Bound {

    /// Get the exclusion box of a block, this function doesn't take the block metadata.
    /// 
    /// PARITY: The Notchian implementation is terrible because it uses the colliding box
    /// for the exclusion box but with the metadata of the block currently at the 
    /// position, so we fix this in this implementation by just returning a full block
    /// for blocks that usually depends on metadata (such as doors, trapdoors).
    fn get_block_exclusion_box(&self, pos: IVec3, id: u8) -> Option<BoundingBox>;

    /// Get the overlay box of the block, this overlay is what should be shown client-side
    /// around the block and where the player can click. Unlike colliding boxes, there is
    /// only one overlay box per block.
    /// 
    /// **Note that** liquid blocks returns no box.
    fn get_block_overlay_box(&self, pos: IVec3, id: u8, metadata: u8) -> Option<BoundingBox>;

    /// Get the colliding boxes for a block, the colliding box will be offset to the 
    /// block's position as needed. Not to confuse with overlay boxes, which are just used
    /// to client side placement rendering, and used server-side to compute ray tracing 
    /// when using items such as bucket.
    fn iter_block_colliding_boxes(&self, pos: IVec3, id: u8, metadata: u8) -> impl Iterator<Item = BoundingBox> + '_
    where Self: Sized;
    
    /// Get the colliding box for a block, this returns a single bounding box that is an
    /// union between all boxes returned by [`iter_block_colliding_boxes`] iterator.
    /// 
    /// [`iter_block_colliding_boxes`]: Self::iter_block_colliding_boxes
    fn get_block_colliding_box(&self, pos: IVec3, id: u8, metadata: u8) -> Option<BoundingBox>
    where Self: Sized {
        let mut iter = self.iter_block_colliding_boxes(pos, id, metadata);
        let mut bb = iter.next()?;
        while let Some(other) = iter.next() {
            bb |= other;
        }
        Some(bb)
    }

    /// Iterate over all blocks that are in the bounding box area, this doesn't check for
    /// actual collision with the block's bounding box, it just return all potential 
    /// blocks in the bounding box' area.
    fn iter_blocks_in_box(&self, bb: BoundingBox) -> impl Iterator<Item = (IVec3, u8, u8)> + '_
    where Self: Sized;

    /// Iterate over all bounding boxes in the given area.
    /// *Min is inclusive and max is exclusive.*
    fn iter_blocks_boxes_in(&self, min: IVec3, max: IVec3) -> impl Iterator<Item = BoundingBox> + '_
    where Self: Sized;

    /// Iterate over all bounding boxes in the given area that are colliding with the 
    /// given one.
    fn iter_blocks_boxes_colliding(&self, bb: BoundingBox) -> impl Iterator<Item = BoundingBox> + '_ 
    where Self: Sized {
        let min = bb.min.floor().as_ivec3();
        let max = bb.max.add(1.0).floor().as_ivec3();
        self.iter_blocks_boxes_in(min, max)
            .filter(move |block_bb| block_bb.intersects(bb))
    }

    /// Ray trace from an origin point and return the first colliding blocks, either 
    /// entity or block. The fluid argument is used to hit the fluid **source** blocks or
    /// not. The overlay argument is used to select the block overlay box instead of the
    /// block bound box.
    fn ray_trace_blocks(&self, origin: DVec3, ray: DVec3, kind: RayTraceKind) -> Option<RayTraceHit>;

}

/// Describe the kind of ray tracing to make, this describe how blocks are collided.
pub enum RayTraceKind {
    /// The ray trace will be on block colliding boxes.
    Colliding,
    /// The ray trace will be on block overlay boxes.
    Overlay,
    /// The ray trace will be on block overlay boxes including fluid sources.
    OverlayWithFluid,
}

/// Result of a ray trace that hit a block.
#[derive(Debug, Clone)]
pub struct RayTraceHit {
    /// The ray vector that stop on the block.
    pub ray: DVec3,
    /// The position of the block.
    pub pos: IVec3,
    /// The block.
    pub block: u8,
    /// The block metadata.
    pub metadata: u8,
    /// The face of the block.
    pub face: Face,
}

