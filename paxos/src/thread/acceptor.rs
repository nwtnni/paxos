use std::collections::HashMap as Map;

use serde_derive::{Serialize, Deserialize};
use tokio::prelude::*;

use crate::message;
use crate::thread::Rx;
use crate::thread::peer;
use crate::shared;
use crate::state;
use crate::storage;

#[derive(Debug)]
pub enum In<C: state::Command> {
    P1A(message::P1A),
    P2A(message::CommanderID, message::P2A<C>),
}

pub struct Acceptor<S: state::State> {
    id: usize,
    stable: Stable<S>,
    storage: storage::Storage<Stable<S>>,
    rx: Rx<In<S::Command>>,
    shared_tx: shared::Shared<S>,
}

#[derive(Serialize, Deserialize)]
#[serde(bound(serialize = "", deserialize = ""))]
#[derive(Derivative)]
#[derivative(Default(bound = ""))]
struct Stable<S: state::State> {
    ballot: message::Ballot,
    accepted: Map<usize, message::PValue<S::Command>>,
}

impl<S: state::State> Future for Acceptor<S> {
    type Item = ();
    type Error = ();
    fn poll(&mut self) -> Result<Async<Self::Item>, Self::Error> {
        while let Async::Ready(Some(message)) = self.rx.poll()? {
            trace!("received {:?}", message);
            match message {
            | In::P1A(m) => self.send_p1a(m),
            | In::P2A(c_id, m) => self.send_p2a(c_id, m),
            }
        }
        Ok(Async::NotReady)
    }
}

impl<S: state::State> Acceptor<S> {

    pub fn new(id: usize, rx: Rx<In<S::Command>>, shared_tx: shared::Shared<S>) -> Self {
        let storage = storage::Storage::new(format!("acceptor-{:>02}.paxos", id));
        let stable = storage.load().unwrap_or_default();
        Acceptor {
            id, 
            stable,
            storage,
            rx,
            shared_tx
        }
    }

    fn send_p1a(&mut self, ballot: message::P1A) {
        self.stable.ballot = std::cmp::max(ballot, self.stable.ballot);
        self.storage.save(&self.stable);
        let p1b = peer::In::P1B(message::P1B {
            a_id: self.id,
            b_id: self.stable.ballot,
            pvalues: self.stable.accepted.values()
                .cloned()
                .collect(),
        });
        trace!("sending {:?} to {}", p1b, ballot.l_id);
        self.shared_tx.read().send(ballot.l_id, p1b)
    }

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
            }
        );
        trace!("sending {:?} to {}", p2b, pvalue.b_id.l_id);
        self.shared_tx.read().send(pvalue.b_id.l_id, p2b)
    }
}
