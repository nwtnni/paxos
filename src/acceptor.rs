use hashbrown::HashMap as Map;
use futures::sync::mpsc;

use tokio::prelude::*;

use crate::message;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum In<O> {
    P1A(message::P1A),
    P2A(message::P2A<O>),
}

pub type Out<O> = (usize, message::Out<O>);

#[derive(Debug)]
pub struct Acceptor<O> {
    id: usize,
    ballot: message::BallotID,
    accepted: Map<usize, message::PValue<O>>,
    rx: mpsc::UnboundedReceiver<In<O>>,
    tx: mpsc::UnboundedSender<Out<O>>,
}

impl<O: Clone + Eq + std::hash::Hash> Acceptor<O> {
    pub async fn run(mut self) {
        while let Some(Ok(message)) = await!(self.rx.next()) {
            let outgoing = match message {
            | In::P1A(m) => (m.l_id, self.respond_p1a(m)),
            | In::P2A(m) => (m.b_id.l_id, self.respond_p2a(m)),
            };
            self.tx.unbounded_send(outgoing)
                .expect("[INTERNAL ERROR]: could not send from acceptor");
        }
    }

    fn respond_p1a(&mut self, ballot: message::P1A) -> message::Out<O> {
        self.ballot = std::cmp::max(ballot, self.ballot);
        message::Out::P1B(message::P1B {
            a_id: self.id,
            b_id: self.ballot,
            pvalues: self.accepted.values()
                .cloned()
                .collect(),
        })
    }

    fn respond_p2a(&mut self, pvalue: message::P2A<O>) -> message::Out<O> {
        if pvalue.b_id >= self.ballot {
            self.ballot = pvalue.b_id;
            self.accepted.insert(pvalue.s_id, pvalue.clone());
        }
        message::Out::P2B(message::P2B {
            a_id: self.id,
            b_id: self.ballot,
        })
    }
}
