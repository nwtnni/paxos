use std::sync::Arc;

use hashbrown::HashMap as Map;
use parking_lot::{RwLock, RwLockReadGuard, RwLockWriteGuard};

use crate::message;
use crate::state;
use crate::thread::{commander, peer, replica, scout, Tx};

#[derive(Derivative)]
#[derivative(Clone(bound = ""))]
pub struct Shared<S: state::State>(Arc<RwLock<State<S>>>);

impl<S: state::State> Shared<S> {
    pub fn new(
        scout_tx: Tx<scout::In<S::Command>>,
        replica_tx: Tx<replica::In<S::Command>>
    ) -> Self {
        Shared(Arc::new(RwLock::new(State::new(scout_tx, replica_tx))))
    }

    pub fn read(&self) -> RwLockReadGuard<State<S>> {
        self.0.read()
    }

    pub fn write(&self) -> RwLockWriteGuard<State<S>> {
        self.0.write()
    }
}

pub struct State<S: state::State> {
    peer_txs: Map<usize, Tx<peer::In<S::Command>>>,
    client_txs: Map<<S::Command as state::Command>::ClientID, Tx<S::Response>>,
    commander_txs: Map<message::CommanderID, Tx<commander::In>>,
    scout_tx: Tx<scout::In<S::Command>>,
    replica_tx: Tx<replica::In<S::Command>>,
}

impl<S: state::State> State<S> {
    pub fn new(
        scout_tx: Tx<scout::In<S::Command>>,
        replica_tx: Tx<replica::In<S::Command>>
    ) -> Self {
        State {
            peer_txs: Map::default(),
            client_txs: Map::default(),
            commander_txs: Map::default(),
            scout_tx,
            replica_tx,
        }
    }

    pub fn replica_tx(&self) -> &Tx<replica::In<S::Command>> {
        &self.replica_tx
    }

    pub fn connect_peer(&mut self, id: usize, tx: Tx<peer::In<S::Command>>) {
        self.peer_txs.insert(id, tx);
    }

    pub fn disconnect_peer(&mut self, id: usize) {
        self.peer_txs.remove(&id);
    }

    pub fn connect_client(&mut self, id: <S::Command as state::Command>::ClientID, tx: Tx<S::Response>) {
        self.client_txs.insert(id, tx);
    }

    pub fn disconnect_client(&mut self, id: <S::Command as state::Command>::ClientID) {
        self.client_txs.remove(&id);
    }

    pub fn connect_commander(&mut self, id: message::CommanderID, tx: Tx<commander::In>) {
        self.commander_txs.insert(id, tx);
    }

    pub fn disconnect_commander(&mut self, id: message::CommanderID) {
        self.commander_txs.remove(&id);
    }

    pub fn replace_scout(&mut self, tx: Tx<scout::In<S::Command>>) {
        std::mem::replace(&mut self.scout_tx, tx);
    }

    pub fn send_commander(&self, c_id: message::CommanderID, message: commander::In) {
        if let Some(tx) = self.commander_txs.get(&c_id) {
            tx.unbounded_send(message)
                .expect("[INTERNAL ERROR]: failed to send to commander");
        }
    }

    pub fn send_replica(&self, message: replica::In<S::Command>) {
        self.replica_tx.unbounded_send(message)
            .expect("[INTERNAL ERROR]: failed to send to replica");
    }

    pub fn send_scout(&self, message: scout::In<S::Command>) {
        self.scout_tx.unbounded_send(message)
            .expect("[INTERNAL ERROR]: failed to send to replica");
    }

    pub fn send_client(&self, id: <S::Command as state::Command>::ClientID, message: S::Response) {
        if let Some(tx) = self.client_txs.get(&id) {
            let _ = tx.unbounded_send(message);
        }
    }

    pub fn send(&self, id: usize, message: peer::In<S::Command>) {
        if let Some(tx) = self.peer_txs.get(&id) {
            let _ = tx.unbounded_send(message);
        }
    }

    pub fn broadcast(&self, message: peer::In<S::Command>) {
        for id in self.peer_txs.keys() {
            self.send(*id, message.clone());
        }
    }

    pub fn narrowcast<'a, T>(&self, ids: T, message: peer::In<S::Command>)
        where T: IntoIterator<Item = &'a usize>
    {
        for id in ids.into_iter() {
            self.send(*id, message.clone());
        }
    }
}
