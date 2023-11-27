//! The module provides base structures for loading and saving chunk data, entities and 
//! block entities within a world. Examples of world sources includes region file loader
//! and saver, or world generators.

use std::sync::Arc;
use std::thread;

use crossbeam_channel::{bounded, Sender, Receiver, TrySendError, TryRecvError};
use thiserror::Error;
use glam::IVec3;

use crate::world::ChunkSnapshot;
use crate::block;


/// Chunk source trait to implement on world loaders/savers or generators, it provides 
/// synchronous methods to load and save, note that if you want to use this trait
/// asynchronously, you should wrap it in a [`ChunkSourcePool`] structure.
pub trait ChunkSource {

    /// The error type when loading a chunk.
    type LoadError;

    /// The error type when saving a chunk.
    type SaveError;

    /// Load a chunk at the given coordinates and return the associated proto-chunk. This
    /// function should load the chunk synchronously. The default implementation just
    /// return an unsupported operation error.
    fn load(&mut self, cx: i32, cz: i32) -> Result<ChunkSnapshot, ChunkSourceError<Self::LoadError>> {
        let _ = (cx, cz);
        Err(ChunkSourceError::Unsupported)
    }

    /// Save a chunk from the given view and return Ok if successful. This function should
    /// save the chunk synchronously. The default implementation just return an
    /// unsupported operation error.
    fn save(&mut self, snapshot: ChunkSnapshot) -> Result<(), ChunkSourceError<Self::SaveError>> {
        let _ = snapshot;
        Err(ChunkSourceError::Unsupported)
    }

}


/// Base error type common to loading and saving of chunks on [`ChunkSource`] trait. It
/// is used to inform the caller if the source support the requested function.
#[derive(Error, Debug)]
pub enum ChunkSourceError<E> {
    /// The request chunk operation is not currently supported. When loading a chunk, 
    /// this could mean either that the chunk is absent from that source or just that
    /// this type of source doesn't support the "load" function. When saving a chunk,
    /// this could mean that the source doesn't support "save" function, or that the
    /// given chunk could not be saved in that source.
    #[error("The operation is not supported.")]
    Unsupported,
    /// All other custom source error goes into this.
    #[error("{0}")]
    Other(#[from] E),
}


/// A chunk source that produces a empty chunk everywhere.
#[derive(Clone, Copy)]
pub struct EmptyChunkSource;
impl ChunkSource for EmptyChunkSource {

    type LoadError = ();
    type SaveError = ();

    fn load(&mut self, cx: i32, cz: i32) -> Result<ChunkSnapshot, ChunkSourceError<Self::LoadError>> {
        Ok(ChunkSnapshot::new(cx, cz))
    }

}

/// A chunk source that generates a flat chunk.
#[derive(Clone, Copy)]
pub struct FlatChunkSource;
impl ChunkSource for FlatChunkSource {

    type LoadError = ();
    type SaveError = ();

    fn load(&mut self, cx: i32, cz: i32) -> Result<ChunkSnapshot, ChunkSourceError<Self::LoadError>> {
        let mut view = ChunkSnapshot::new(cx, cz);
        let chunk = Arc::get_mut(&mut view.chunk).unwrap();
        chunk.fill_block(IVec3::new(0, 0, 0), IVec3::new(16, 1, 16), block::BEDROCK, 0);
        chunk.fill_block(IVec3::new(0, 1, 0), IVec3::new(16, 3, 16), block::DIRT, 0);
        chunk.fill_block(IVec3::new(0, 4, 0), IVec3::new(16, 1, 16), block::GRASS, 0);
        Ok(view)
    }

}

// /// A chunk source wrapper type that takes two chunk sources, operations are first tried
// /// on the first source but fallback to the second one if the error returned is
// /// [`ChunkSourceError::Unsupported`].
// #[derive(Debug, Clone)]
// pub struct FallbackChunkSource<S0: ChunkSource, S1: ChunkSource>(pub S0, pub S1);
// impl<S0: ChunkSource, S1: ChunkSource> ChunkSource for FallbackChunkSource<S0, S1> {
    
//     type LoadError;
//     type SaveError;
    
//     fn load_chunk(&mut self, cx: i32, cz: i32) -> Result<ChunkView, ChunkSourceError<Self::LoadError>> {
//         let _ = (cx, cz);
//         Err(ChunkSourceError::Unsupported)
//     }

//     fn save_chunk(&mut self, view: ChunkView) -> Result<(), ChunkSourceError<Self::SaveError>> {
//         let _ = view;
//         Err(ChunkSourceError::Unsupported)
//     }
    
// }


/// This wrapper allows wrapping chunk source(s) in thread(s) in order to asynchronously
/// load and save chunks through worker threads, all workers return the result to this
/// pool object.
pub struct ChunkSourcePool<S: ChunkSource> {
    command_sender: Sender<WorkerCommand>,
    event_receiver: Receiver<ChunkSourceEvent<S>>,
}

impl<S: ChunkSource> ChunkSourcePool<S> {

    /// Internal utility to construct the threaded source with given workers count to 
    /// adapt the bounds of command and result channels.
    fn new_internal<F>(workers_count: usize, start_threads: F) -> Self
    where
        F: FnOnce(Receiver<WorkerCommand>, Sender<ChunkSourceEvent<S>>),
    {

        let (
            command_sender,
            command_receiver,
        ) = bounded(workers_count * 100);

        let (
            event_sender,
            event_receiver,
        ) = bounded(workers_count * 100);

        start_threads(command_receiver, event_sender);
        
        Self {
            command_sender,
            event_receiver,
        }

    }

    /// Create a threaded source with multiple worker thread that receives commands.
    pub fn new(source: S, workers_count: usize) -> Self
    where
        S: Send + Clone + 'static,
        S::LoadError: Send,
        S::SaveError: Send,
    {
        Self::new_internal(workers_count, |command_receiver, event_sender| {

            let mut source = Some(source);
            for i in 0..workers_count {

                // In order to not clone the original source.
                let source = if i != workers_count - 1 {
                    source.as_ref().unwrap().clone()
                } else {
                    source.take().unwrap()
                };

                let command_receiver = command_receiver.clone();
                let event_sender = event_sender.clone();

                thread::Builder::new()
                    .name(format!("Chunk Source Thread #{i}"))
                    .spawn(move || {
                        Worker::<S> { 
                            inner: source, 
                            command_receiver, 
                            event_sender,
                        }.run()
                    })
                    .unwrap();

            }

        })
    }

    /// Create a threaded source with a single worker thread that receives commands.
    pub fn new_single(source: S) -> Self
    where
        S: Send + 'static,
        S::LoadError: Send,
        S::SaveError: Send,
    {
        Self::new_internal(1, |command_receiver, event_sender| {
            thread::Builder::new()
                .name(format!("Chunk Source Thread"))
                .spawn(move || {
                    Worker::<S> { 
                        inner: source, 
                        command_receiver, 
                        event_sender,
                    }.run()
                })
                .unwrap();
        })
    }

    /// Request a chunk to be loaded, this function returns true if the request has been
    /// successfully enqueued.
    pub fn request_chunk_load(&self, cx: i32, cz: i32) -> bool {
        match self.command_sender.try_send(WorkerCommand::Load { cx, cz }) {
            Ok(_) => true,
            Err(TrySendError::Full(_)) => false,
            Err(TrySendError::Disconnected(_)) => panic!("worker thread should not disconnect"),
        }
    }

    /// Request a chunk to be saved from a view. This function returns true if the request
    /// has been successfully enqueued.
    pub fn request_chunk_save(&self, snapshot: ChunkSnapshot) -> bool {
        match self.command_sender.try_send(WorkerCommand::Save { view: snapshot }) {
            Ok(_) => true,
            Err(TrySendError::Full(_)) => false,
            Err(TrySendError::Disconnected(_)) => panic!("worker thread should not disconnect"),
        }
    }

    /// Poll the next available event from this source, this provides feedback from 
    /// chunk source workers when loading or saving chunks.
    pub fn poll_event(&self) -> Option<ChunkSourceEvent<S>> {
        loop {
            match self.event_receiver.try_recv() {
                Ok(event) => break Some(event),
                Err(TryRecvError::Empty) => break None,
                Err(TryRecvError::Disconnected) => panic!("worker thread should not disconnect"),
            }
        }
    }

}

/// Internal enumeration of results to commands sent to workers.
pub enum ChunkSourceEvent<S: ChunkSource> {
    /// A chunk has been loaded, here is the chunk snapshot or an error.
    Load(Result<ChunkSnapshot, ChunkSourceError<S::LoadError>>),
    /// A chunk has been saved, here is the chunk coordinates or an error.
    Save(Result<(i32, i32), ChunkSourceError<S::SaveError>>),
}

/// Inner structure that contains a worker 
struct Worker<S: ChunkSource> {
    inner: S,
    command_receiver: Receiver<WorkerCommand>,
    event_sender: Sender<ChunkSourceEvent<S>>,
}

/// Internal enumeration of commands that are sent to the threaded sources.
enum WorkerCommand {
    /// Load a chunk at the given coordinates.
    Load { cx: i32, cz: i32 },
    /// Save a chunk from its view.
    Save { view: ChunkSnapshot },
}

impl<S: ChunkSource> Worker<S> {

    /// Internal function to run the worker until the commands channel is disconnected.
    fn run(mut self) {
        while let Ok(command) = self.command_receiver.recv() {
            match command {
                WorkerCommand::Load { cx, cz } => {
                    let result = self.inner.load(cx, cz);
                    // NOTE: We block until it's possible to send, but if the channel is
                    // disconnected, this means that the receiver side has been 
                    // disconnected, we should shutdown.
                    if self.event_sender.send(ChunkSourceEvent::Load(result)).is_err() {
                        break;
                    }
                }
                WorkerCommand::Save { view } => {
                    let (cx, cz) = (view.cx, view.cz);
                    let result = self.inner.save(view);
                    if self.event_sender.send(ChunkSourceEvent::Save(result.map(|_| (cx, cz)))).is_err() {
                        break;
                    }
                }
            }
        }
    }

}
