use hashbrown::HashMap as Map;
use tokio::prelude::*;

use crate::message;
use crate::thread::Rx;
use crate::thread::peer;
use crate::shared;
use crate::state;

#[derive(Debug)]
pub enum In<C: state::Command> {
    P1A(message::P1A),
    P2A(message::CommanderID, message::P2A<C>),
}

pub struct Acceptor<S: state::State> {
    id: usize,
    ballot: message::BallotID,
    accepted: Map<usize, message::PValue<S::Command>>,
    rx: Rx<In<S::Command>>,
    shared_tx: shared::Shared<S>,
}

impl<S: state::State> Future for Acceptor<S> {
    type Item = ();
    type Error = ();
    fn poll(&mut self) -> Result<Async<Self::Item>, Self::Error> {
        while let Async::Ready(Some(message)) = self.rx.poll()? {
            trace!("received message {:?}", message);
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
        Acceptor {
            id, 
            ballot: message::BallotID::default(),
            accepted: Map::default(),
            rx,
            shared_tx
        }
    }

    fn send_p1a(&mut self, ballot: message::P1A) {
        self.ballot = std::cmp::max(ballot, self.ballot);
        let p1b = peer::In::P1B(message::P1B {
            a_id: self.id,
            b_id: self.ballot,
            pvalues: self.accepted.values()
                .cloned()
                .collect(),
        });
        trace!("sending message {:?} to {}", p1b, ballot.l_id);
        self.shared_tx.read().send(ballot.l_id, p1b)
    }

    fn send_p2a(&mut self, c_id: message::CommanderID, pvalue: message::P2A<S::Command>) {
        if pvalue.b_id >= self.ballot {
            self.ballot = pvalue.b_id;
            self.accepted.insert(pvalue.s_id, pvalue.clone());
        }
        let p2b = peer::In::P2B(
            c_id,
            message::P2B {
                a_id: self.id,
                b_id: self.ballot,
            }
        );
        trace!("sending message {:?} to {}", p2b, pvalue.b_id.l_id);
        self.shared_tx.read().send(pvalue.b_id.l_id, p2b)
    }
}
