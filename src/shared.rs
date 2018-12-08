use std::sync::Arc;

use hashbrown::HashMap as Map;
use parking_lot::{RwLock, RwLockReadGuard, RwLockWriteGuard};

use crate::thread::{commander, scout, peer, Tx};

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
pub struct State<O> {
    peer_txs: Map<usize, Tx<peer::In<O>>>,
    commander_txs: Map<commander::ID, Tx<commander::In>>,
    scout_tx: Tx<scout::In<O>>,
}

impl<O: Clone> State<O> {
    pub fn connect_peer(&mut self, id: usize, tx: Tx<peer::In<O>>) {
        self.peer_txs.insert(id, tx);
    }

    pub fn disconnect_peer(&mut self, id: usize) {
        self.peer_txs.remove(&id);
    }

    pub fn connect_commander(&mut self, id: commander::ID, tx: Tx<commander::In>) {
        self.commander_txs.insert(id, tx);
    }

    pub fn disconnect_commander(&mut self, id: commander::ID) {
        self.commander_txs.remove(&id);
    }

    pub fn replace_scout(&mut self, tx: Tx<scout::In<O>>) {
        std::mem::replace(&mut self.scout_tx, tx);
    }

    pub fn send(&self, id: usize, message: peer::In<O>) {
        if let Some(tx) = self.peer_txs.get(&id) {
            let _ = tx.unbounded_send(message);
        }
    }

    pub fn broadcast(&self, message: peer::In<O>) {
        for id in self.peer_txs.keys() {
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
