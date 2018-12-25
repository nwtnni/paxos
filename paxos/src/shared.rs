//! # Summary
//!
//! This module implements a central hub for intra-server message
//! forwarding. We wrap the central `State` type with Arc<RwLock<T>>
//! to share the connections between concurrently running threads.

use std::collections::HashMap as Map;
use std::sync::Arc;

use parking_lot::{RwLock, RwLockReadGuard, RwLockWriteGuard};

use crate::message;
use crate::state;
use crate::thread::{acceptor, commander, peer, replica, scout, Tx};

/// Thread-safe wrapper around `State` forwarding hub.
#[derive(Derivative)]
#[derivative(Clone(bound = ""))]
pub struct Shared<S: state::State>(Arc<RwLock<State<S>>>);

impl<S: state::State> Shared<S> {

    /// Initializes a message hub with the provided transmission channels.
    pub fn new(
        id: usize,
        scout_tx: Tx<scout::In<S::Command>>,
        replica_tx: Tx<replica::In<S::Command>>,
        acceptor_tx: Tx<acceptor::In<S::Command>>,
    ) -> Self {
        Shared(Arc::new(RwLock::new(
            State::new(id, scout_tx, replica_tx, acceptor_tx)
        )))
    }

    /// Acquires a read lock on the underlying state.
    pub fn read(&self) -> RwLockReadGuard<State<S>> {
        self.0.read()
    }

    /// Acquires a write lock on the underlying state.
    pub fn write(&self) -> RwLockWriteGuard<State<S>> {
        self.0.write()
    }
}

/// Collection of intra-server transmitting channels.
pub struct State<S: state::State> {
    id: usize,
    peer_txs: Map<usize, Tx<peer::In<S::Command>>>,
    client_txs: Map<<S::Command as state::Command>::ClientID, Tx<S::Response>>,
    commander_txs: Map<message::CommanderID, Tx<commander::In>>,
    scout_tx: Tx<scout::In<S::Command>>,
    replica_tx: Tx<replica::In<S::Command>>,
    acceptor_tx: Tx<acceptor::In<S::Command>>,
}

impl<S: state::State> State<S> {

    /// Initializes a message hub with the provided transmission channels.
    pub fn new(
        id: usize,
        scout_tx: Tx<scout::In<S::Command>>,
        replica_tx: Tx<replica::In<S::Command>>,
        acceptor_tx: Tx<acceptor::In<S::Command>>,
    ) -> Self {
        State {
            id,
            peer_txs: Map::default(),
            client_txs: Map::default(),
            commander_txs: Map::default(),
            scout_tx,
            replica_tx,
            acceptor_tx,
        }
    }

    /// Registers the provided peer channel with this hub.
    pub fn connect_peer(&mut self, id: usize, tx: Tx<peer::In<S::Command>>) {
        self.peer_txs.insert(id, tx);
    }

    /// Disconnects the provided peer from this hub.
    pub fn disconnect_peer(&mut self, id: usize) {
        self.peer_txs.remove(&id);
    }

    /// Registers the provided client channel with this hub.
    pub fn connect_client(&mut self, id: <S::Command as state::Command>::ClientID, tx: Tx<S::Response>) {
        self.client_txs.insert(id, tx);
    }

    /// Disconnects the provided client from this hub.
    pub fn disconnect_client(&mut self, id: &<S::Command as state::Command>::ClientID) {
        self.client_txs.remove(id);
    }

    /// Registers the provided commander with this hub.
    pub fn connect_commander(&mut self, id: message::CommanderID, tx: Tx<commander::In>) {
        self.commander_txs.insert(id, tx);
    }

    /// Disconnects the provided commander from this hub.
    pub fn disconnect_commander(&mut self, id: message::CommanderID) {
        self.commander_txs.remove(&id);
    }

    /// Replaces the scout channel associated with this hub.
    pub fn replace_scout(&mut self, tx: Tx<scout::In<S::Command>>) {
        std::mem::replace(&mut self.scout_tx, tx);
    }

    /// Forwards a message to the provided commander.
    pub fn send_commander(&self, c_id: message::CommanderID, message: commander::In) {
        if let Some(tx) = self.commander_txs.get(&c_id) {
            // Commander may have received majority and dropped receiving end
            tx.unbounded_send(message).ok();
        }
    }

    /// Forwards a message to the replica sub-thread.
    pub fn send_replica(&self, message: replica::In<S::Command>) {
        self.replica_tx.unbounded_send(message)
            .expect("[INTERNAL ERROR]: failed to send to replica");
    }

    /// Forwards a message to the scout sub-thread.
    pub fn send_scout(&self, message: scout::In<S::Command>) {
        // Scout might have received a majority and dropped receiving end
        self.scout_tx.unbounded_send(message).ok();
    }

    /// Forwards a message to the acceptor sub-thread.
    pub fn send_acceptor(&self, message: acceptor::In<S::Command>) {
        self.acceptor_tx.unbounded_send(message)
            .expect("[INTERNAL ERROR]: failed to send to acceptor");
    }

    /// Forwards a message to an external client.
    pub fn send_client(&self, id: <S::Command as state::Command>::ClientID, message: S::Response) {
        if let Some(tx) = self.client_txs.get(&id) {
            let _ = tx.unbounded_send(message);
        }
    }

    /// Forwards a message to an external peer server.
    pub fn send(&self, id: usize, message: peer::In<S::Command>) {
        if id == self.id {
            self.forward(message);
        } else if let Some(tx) = self.peer_txs.get(&id) {
            let _ = tx.unbounded_send(message);
        }
    }

    /// Forwards a message within this process.
    pub fn forward(&self, message: peer::In<S::Command>) {
        match message {
        | peer::In::P1A(p1a) => self.send_acceptor(acceptor::In::P1A(p1a)),
        | peer::In::P1B(p1b) => self.send_scout(p1b),
        | peer::In::P2A(c_id, p2a) => self.send_acceptor(acceptor::In::P2A(c_id, p2a)),
        | peer::In::P2B(c_id, p2b) => self.send_commander(c_id, p2b),
        | peer::In::Decision(proposal) => self.send_replica(replica::In::Decision(proposal)),
        | peer::In::Ping(_) => (),
        }
    }

    /// Forwards a message to the provided list of peer servers.
    pub fn narrowcast<'a, T>(&self, ids: T, message: peer::In<S::Command>)
        where T: IntoIterator<Item = &'a usize>
    {
        for id in ids.into_iter() {
            self.send(*id, message.clone());
        }
    }

    /// Forwards a message to all connected peer servers.
    pub fn broadcast(&self, message: peer::In<S::Command>) {
        for tx in self.peer_txs.values() {
            let _ = tx.unbounded_send(message.clone());
        }
        self.forward(message);
    }
}
