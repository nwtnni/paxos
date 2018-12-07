use hashbrown::HashMap as Map;
use futures::sync::mpsc;

use tokio::prelude::*;

use crate::message;
use crate::thread::forward;

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
    tx: mpsc::UnboundedSender<forward::In<O>>,
}

impl<O: Clone + Eq + std::hash::Hash> Acceptor<O> {
    pub async fn run(mut self) {
        while let Some(Ok(message)) = await!(self.rx.next()) {
            let out = match message {
            | In::P1A(m) => self.respond_p1a(m),
            | In::P2A(m) => self.respond_p2a(m),
            };
            self.tx.unbounded_send(out)
                .expect("[INTERNAL ERROR]: could not send from acceptor");
        }
    }

    fn respond_p1a(&mut self, ballot: message::P1A) -> forward::In<O> {
        self.ballot = std::cmp::max(ballot, self.ballot);
        forward::In::P1B(ballot.l_id, message::P1B {
            a_id: self.id,
            b_id: self.ballot,
            pvalues: self.accepted.values()
                .cloned()
                .collect(),
        })
    }

    fn respond_p2a(&mut self, pvalue: message::P2A<O>) -> forward::In<O> {
        if pvalue.b_id >= self.ballot {
            self.ballot = pvalue.b_id;
            self.accepted.insert(pvalue.s_id, pvalue.clone());
        }
        forward::In::P2B(pvalue.b_id.l_id, message::P2B {
            a_id: self.id,
            b_id: self.ballot,
        })
    }
}
