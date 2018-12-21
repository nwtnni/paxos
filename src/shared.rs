use std::sync::Arc;

use hashbrown::HashMap as Map;
use parking_lot::{RwLock, RwLockReadGuard, RwLockWriteGuard};

use crate::thread::{commander, peer, replica, scout, Tx};

#[derive(Debug, Clone)]
pub struct Shared<I>(Arc<RwLock<State<I>>>);

impl<I> Shared<I> {
    pub fn new(scout_tx: Tx<scout::In<I>>, replica_tx: Tx<replica::In<I>>) -> Self {
        Shared(Arc::new(RwLock::new(State::new(scout_tx, replica_tx))))
    }

    pub fn read(&self) -> RwLockReadGuard<State<I>> {
        self.0.read()
    }

    pub fn write(&self) -> RwLockWriteGuard<State<I>> {
        self.0.write()
    }
}

#[derive(Debug)]
pub struct State<I> {
    peer_txs: Map<usize, Tx<peer::In<I>>>,
    commander_txs: Map<commander::ID, Tx<commander::In>>,
    scout_tx: Tx<scout::In<I>>,
    replica_tx: Tx<replica::In<I>>,
}

impl<I> State<I> {
    pub fn new(scout_tx: Tx<scout::In<I>>, replica_tx: Tx<replica::In<I>>) -> Self {
        State {
            peer_txs: Map::default(),
            commander_txs: Map::default(),
            scout_tx,
            replica_tx,
        }
    }

    pub fn replica_tx(&self) -> &Tx<replica::In<I>> {
        &self.replica_tx
    }

    pub fn connect_peer(&mut self, id: usize, tx: Tx<peer::In<I>>) {
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

    pub fn replace_scout(&mut self, tx: Tx<scout::In<I>>) {
        std::mem::replace(&mut self.scout_tx, tx);
    }

    pub fn send_commander(&self, c_id: commander::ID, message: commander::In) {
        if let Some(tx) = self.commander_txs.get(&c_id) {
            tx.unbounded_send(message)
                .expect("[INTERNAL ERROR]: failed to send to commander");
        }
    }

    pub fn send_replica(&self, message: replica::In<I>) {
        self.replica_tx.unbounded_send(message)
            .expect("[INTERNAL ERROR]: failed to send to replica");
    }

    pub fn send_scout(&self, message: scout::In<I>) {
        self.scout_tx.unbounded_send(message)
            .expect("[INTERNAL ERROR]: failed to send to replica");
    }

    pub fn send(&self, id: usize, message: peer::In<I>) {
        if let Some(tx) = self.peer_txs.get(&id) {
            let _ = tx.unbounded_send(message);
        }
    }
}

impl<I: Clone> State<I> {
    pub fn broadcast(&self, message: peer::In<I>) {
        for id in self.peer_txs.keys() {
            self.send(*id, message.clone());
        }
    }

    pub fn narrowcast<'a, T>(&self, ids: T, message: peer::In<I>)
        where T: IntoIterator<Item = &'a usize>
    {
        for id in ids.into_iter() {
            self.send(*id, message.clone());
        }
    }
}
