//! # Summary
//!
//! This module defines the `Acceptor` struct, which acts as Paxos's
//! distributed memory. Acceptors keep track of what commands have been
//! proposed for each slot.

use std::collections::HashMap as Map;

use serde_derive::{Deserialize, Serialize};
use tokio::prelude::*;

use crate::internal;
use crate::message;
use crate::shared;
use crate::state;
use crate::storage;
use crate::thread::peer;

/// Acceptors can only receive P1A from scouts and P2A from commanders.
#[derive(Debug)]
pub enum In<C: state::Command> {
    P1A(message::P1A),
    P2A(message::CommanderID, message::P2A<C>),
}

/// Functions as distributed memory.
pub struct Acceptor<S: state::State> {
    /// Unique ID of acceptor
    id: usize,

    /// Intra-server receiving channel
    rx: internal::Rx<In<S::Command>>,

    /// Intra-server shared transmitting channels
    shared_tx: shared::Shared<S>,

    /// Persistent acceptor state across failures
    stable: Stable<S>,

    /// Backing store for stable storage
    storage: storage::Storage<Stable<S>>,
}

/// Acceptors keep track of the highest ballot they have seen,
/// and the most recently accepted PValue per slot.
#[derive(Serialize, Deserialize)]
#[serde(bound(serialize = "", deserialize = ""))]
#[derive(Derivative)]
#[derivative(Default(bound = ""))]
struct Stable<S: state::State> {
    /// Highest ballot seen
    ballot: message::Ballot,

    /// Most recently accepted PValue per slot
    accepted: Map<usize, message::PValue<S::Command>>,
}

impl<S: state::State> Future for Acceptor<S> {
    type Item = ();
    type Error = ();
    fn poll(&mut self) -> Result<Async<Self::Item>, Self::Error> {
        while let Async::Ready(Some(message)) = self.rx.poll()? {
            trace!("received {:?}", message);
            match message {
                In::P1A(m) => self.send_p1a(m),
                In::P2A(c_id, m) => self.send_p2a(c_id, m),
            }
        }
        Ok(Async::NotReady)
    }
}

impl<S: state::State> Acceptor<S> {
    /// Initializes a new acceptor with the given transmission channels.
    pub fn new(id: usize, rx: internal::Rx<In<S::Command>>, shared_tx: shared::Shared<S>) -> Self {
        let storage_file = format!("acceptor-{:>02}.paxos", id);
        let storage = storage::Storage::new(storage_file);
        let stable = storage.load().unwrap_or_default();
        Acceptor {
            id,
            stable,
            storage,
            rx,
            shared_tx,
        }
    }

    /// Updates highest ballot seen, and responds to the sending scout with a P1B.
    /// Only sends PValues for slots that the scout doesn't know decisions for.
    fn send_p1a(&mut self, p1a: message::P1A) {
        self.stable.ballot = std::cmp::max(p1a.b_id, self.stable.ballot);
        self.storage.save(&self.stable);
        let pvalues = self.stable.accepted.values()
            .filter(|pvalue| p1a.decided.is_none() || pvalue.s_id > p1a.decided.unwrap())
            .cloned()
            .collect();
        let p1b = peer::In::P1B(message::P1B {
            a_id: self.id,
            b_id: self.stable.ballot,
            pvalues,
        });
        trace!("sending {:?} to {}", p1b, p1a.b_id.l_id);
        self.shared_tx.read().send(p1a.b_id.l_id, p1b)
    }

    /// Updates the map of accepted PValues, and responds to the sending commander with a P2B.
    fn send_p2a(&mut self, c_id: message::CommanderID, pvalue: message::P2A<S::Command>) {
        if pvalue.b_id >= self.stable.ballot {
            self.stable.ballot = pvalue.b_id;
            self.stable.accepted.insert(pvalue.s_id, pvalue.clone());
            self.storage.save(&self.stable);
        }
        let p2b = peer::In::P2B(
            c_id,
            message::P2B {
                a_id: self.id,
                b_id: self.stable.ballot,
            },
        );
        trace!("sending {:?} to {}", p2b, pvalue.b_id.l_id);
        self.shared_tx.read().send(pvalue.b_id.l_id, p2b)
    }
}
