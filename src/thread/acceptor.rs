use hashbrown::HashMap as Map;

use tokio::prelude::*;

use crate::message;
use crate::thread::Rx;
use crate::thread::{commander, peer};
use crate::shared;
use crate::state;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum In<I> {
    P1A(message::P1A),
    P2A(commander::ID, message::P2A<I>),
}

#[derive(Debug)]
pub struct Acceptor<I> {
    id: usize,
    ballot: message::BallotID,
    accepted: Map<usize, message::PValue<I>>,
    rx: Rx<In<I>>,
    shared_tx: shared::Shared<I>,
}

impl<I: state::Identifier> Acceptor<I> {

    pub fn new(id: usize, rx: Rx<In<I>>, shared_tx: shared::Shared<I>) -> Self {
        Acceptor {
            id, 
            ballot: message::BallotID::default(),
            accepted: Map::default(),
            rx,
            shared_tx
        }
    }

    pub async fn run(mut self) {
        loop {
            while let Some(Ok(message)) = await!(self.rx.next()) {
                match message {
                | In::P1A(m) => self.send_p1a(m),
                | In::P2A(c_id, m) => self.send_p2a(c_id, m),
                }
            }
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
        self.shared_tx.read().send(ballot.l_id, p1b)
    }

    fn send_p2a(&mut self, c_id: commander::ID, pvalue: message::P2A<I>) {
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
        self.shared_tx.read().send(pvalue.b_id.l_id, p2b)
    }
}
