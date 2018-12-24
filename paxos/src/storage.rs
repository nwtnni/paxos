use std::io::Seek;

/// Persistent storage for failure recovery.
pub struct Storage<S> {
    storage: std::fs::File,
    _marker: std::marker::PhantomData<S>,
}

impl<S> Storage<S> {
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
    pub fn load(&self) -> Option<S> {
        bincode::deserialize_from(&self.storage).ok()
    }
}

impl<S: serde::Serialize> Storage<S> {
    pub fn save(&mut self, state: &S) {
        self.storage.set_len(0)
            .expect("[STORAGE ERROR]: failed to trim file");
        self.storage.seek(std::io::SeekFrom::Start(0))
            .expect("[STORAGE ERROR]: failed to reset file cursor");
        bincode::serialize_into(&mut self.storage, state)
            .expect("[STORAGE ERROR]: failed to serialize state");
    }
}
