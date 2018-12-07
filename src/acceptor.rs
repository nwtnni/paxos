use hashbrown::HashMap as Map;
use futures::channel::mpsc;
use futures::prelude::*;

use crate::message;
use crate::shared::Shared;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum In<O> {
    P1A(message::P1A),
    P2A(message::P2A<O>),
}

#[derive(Debug)]
pub struct Acceptor<O> {
    id: usize,
    ballot: message::BallotID,
    accepted: Map<usize, message::PValue<O>>,
    rx: mpsc::UnboundedReceiver<In<O>>,
    tx: mpsc::UnboundedSender<message::Out<O>>,
}

impl<O: Clone + Eq + std::hash::Hash> Acceptor<O> {
    pub async fn run(mut self) {
        while let Some(message) = await!(self.rx.next()) {
            let out = match message {
            | In::P1A(m) => self.respond_p1a(m),
            | In::P2A(m) => self.respond_p2a(m),
            };
        }
    }

    fn respond_p1a(&mut self, ballot: message::P1A) -> message::Out<O> {
        self.ballot = std::cmp::max(ballot, self.ballot);
        let p1b = message::P1B {
            a_id: self.id,
            b_id: self.ballot,
            pvalues: self.accepted.values()
                .cloned()
                .collect(),
        };
        message::Out::P1B(p1b)
    }

    fn respond_p2a(&mut self, pvalue: message::P2A<O>) -> message::Out<O> {
        if pvalue.id >= self.ballot {
            self.ballot = pvalue.id;
            self.accepted.insert(pvalue.slot, pvalue.clone());
        }

        let p2b = message::P2B {
            a_id: self.id,
            b_id: self.ballot,
        };

        message::Out::P2B(p2b)
    }
}
