use std::sync::Arc;

use hashbrown::HashMap as Map;
use futures::sync::mpsc;
use parking_lot::{RwLock, RwLockReadGuard, RwLockWriteGuard};

use crate::thread::peer;

#[derive(Debug, Clone)]
pub struct Shared<O>(Arc<RwLock<State<O>>>);

impl<O> Shared<O> {
    pub fn read(&self) -> RwLockReadGuard<State<O>> {
        self.0.read()
    }

    pub fn write(&self) -> RwLockWriteGuard<State<O>> {
        self.0.write()
    }
}

#[derive(Debug)]
pub struct State<O>(Map<usize, mpsc::UnboundedSender<peer::In<O>>>);

impl<O: Clone> State<O> {
    pub fn connect(&mut self, id: usize, tx: mpsc::UnboundedSender<peer::In<O>>) {
        self.0.insert(id, tx);
    }

    pub fn disconnect(&mut self, id: usize) {
        self.0.remove(&id);
    }

    pub fn send(&self, id: usize, message: peer::In<O>) {
        if let Some(tx) = self.0.get(&id) {
            let _ = tx.unbounded_send(message);
        }
    }

    pub fn broadcast(&self, message: peer::In<O>) {
        for id in self.0.keys() {
            self.send(*id, message.clone());
        }
    }

    pub fn narrowcast<'a, I>(&self, ids: I, message: peer::In<O>)
        where I: IntoIterator<Item = &'a usize>
    {
        for id in ids.into_iter() {
            self.send(*id, message.clone());
        }
    }
}
