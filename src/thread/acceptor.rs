use hashbrown::HashMap as Map;

use tokio::prelude::*;

use crate::message;
use crate::thread::Rx;
use crate::thread::peer;
use crate::shared;
use crate::state;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum In<I> {
    P1A(message::P1A),
    P2A(message::P2A<I>),
}

#[derive(Debug)]
pub struct Acceptor<I> {
    id: usize,
    ballot: message::BallotID,
    accepted: Map<usize, message::PValue<I>>,
    rx: Rx<In<I>>,
    tx: shared::Shared<I>,
}

impl<I: state::Identifier> Acceptor<I> {
    pub async fn run(mut self) {
        loop {
            while let Some(Ok(message)) = await!(self.rx.next()) {
                match message {
                | In::P1A(m) => self.send_p1a(m),
                | In::P2A(m) => self.send_p2a(m),
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
        self.tx.read().send(ballot.l_id, p1b)
    }

    fn send_p2a(&mut self, pvalue: message::P2A<I>) {
        if pvalue.b_id >= self.ballot {
            self.ballot = pvalue.b_id;
            self.accepted.insert(pvalue.s_id, pvalue.clone());
        }
        let p2b = peer::In::P2B(message::P2B {
            a_id: self.id,
            b_id: self.ballot,
        });
        self.tx.read().send(pvalue.b_id.l_id, p2b)
    }
}
