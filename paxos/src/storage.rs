//! # Summary
//!
//! This module abstracts over stable storage. To perform failure recovery, Paxos
//! requires that some state persist between failures.
//!
//! Currently uses `bincode` to serialize the necessary state to the filesystem,
//! and to deserialize it when recovering a process. This is a naive, inefficient
//! implementation that clears the file on every write and re-serializes data
//! from scratch.

use std::io::Seek;

/// Persistent storage for failure recovery.
pub struct Storage<S> {
    storage: std::fs::File,
    _marker: std::marker::PhantomData<S>,
}

impl<S> Storage<S> {
    /// Creates a new stable storage file at relative path `path`.
    pub fn new<P: AsRef<std::path::Path>>(path: P) -> Self {
        let storage = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .open(path)
            .expect("[STORAGE ERROR]: could not create stable storage");
        Storage {
            storage,
            _marker: Default::default(),
        }
    }
}

impl<S: serde::de::DeserializeOwned> Storage<S> {
    /// Attempts to load state from disk, returning None if nothing
    /// has been written or an error was encountered during deserialization.
    pub fn load(&self) -> Option<S> {
        bincode::deserialize_from(&self.storage).ok()
    }
}

impl<S: serde::Serialize> Storage<S> {
    /// Saves state to disk. Will panic if modifying the underlying file fails.
    pub fn save(&mut self, state: &S) {
        self.storage.set_len(0)
            .expect("[STORAGE ERROR]: failed to trim file");
        self.storage.seek(std::io::SeekFrom::Start(0))
            .expect("[STORAGE ERROR]: failed to reset file cursor");
        bincode::serialize_into(&mut self.storage, state)
            .expect("[STORAGE ERROR]: failed to serialize state");
    }
}
